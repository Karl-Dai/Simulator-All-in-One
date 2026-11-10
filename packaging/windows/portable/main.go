package main

import (
	"archive/zip"
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

// Populated by generated embeds in the isolated packaging directory.
var payload []byte

var indexJSON []byte

type fileRecord struct {
	SHA256 string `json:"sha256"`
	Size   int64  `json:"size"`
}

func safeName(name string) bool {
	return filepath.IsLocal(name) && name != "." && !strings.ContainsAny(name, "\\:") && filepath.ToSlash(filepath.Clean(name)) == name
}

func verify(root string, index map[string]fileRecord) error {
	info, err := os.Lstat(root)
	if err != nil || !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return fmt.Errorf("runtime folder is missing or invalid: %s", root)
	}
	for name, record := range index {
		if !safeName(name) {
			return fmt.Errorf("invalid payload path: %s", name)
		}
		path := filepath.Join(root, filepath.FromSlash(name))
		for parent := filepath.Dir(path); parent != root; parent = filepath.Dir(parent) {
			st, e := os.Lstat(parent)
			if e != nil || !st.IsDir() || st.Mode()&os.ModeSymlink != 0 {
				return fmt.Errorf("invalid runtime directory: %s", parent)
			}
		}
		st, e := os.Lstat(path)
		if e != nil || !st.Mode().IsRegular() || st.Size() != record.Size {
			return fmt.Errorf("runtime file is missing or changed: %s", path)
		}
		f, e := os.Open(path)
		if e != nil {
			return e
		}
		h := sha256.New()
		_, e = io.Copy(h, f)
		f.Close()
		if e != nil {
			return e
		}
		if hex.EncodeToString(h.Sum(nil)) != record.SHA256 {
			return fmt.Errorf("runtime checksum mismatch: %s", path)
		}
	}
	return filepath.WalkDir(root, func(path string, entry fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.Type()&os.ModeSymlink != 0 {
			return fmt.Errorf("unexpected runtime link: %s", path)
		}
		if !entry.IsDir() {
			name, err := filepath.Rel(root, path)
			if err != nil {
				return err
			}
			if _, ok := index[filepath.ToSlash(name)]; !ok {
				return fmt.Errorf("unexpected runtime file: %s", path)
			}
		}
		return nil
	})
}

func prepare(parent string, archive []byte, index map[string]fileRecord) (string, error) {
	if len(index) == 0 {
		return "", fmt.Errorf("empty payload index")
	}
	for name := range index {
		if !safeName(name) {
			return "", fmt.Errorf("invalid payload path: %s", name)
		}
	}
	id := fmt.Sprintf("%x", sha256.Sum256(archive))
	destination := filepath.Join(parent, id)
	if _, err := os.Lstat(destination); err == nil {
		if err = verify(destination, index); err != nil {
			backup := destination + ".damaged-" + fmt.Sprint(time.Now().UnixNano())
			if moveErr := os.Rename(destination, backup); moveErr != nil {
				return "", fmt.Errorf("close all SAIO windows before repairing the cache: %w", moveErr)
			}
		} else {
			return destination, nil
		}
	} else if !os.IsNotExist(err) {
		return "", err
	}
	if err := os.MkdirAll(parent, 0700); err != nil {
		return "", err
	}
	temporary, err := os.MkdirTemp(parent, ".extract-")
	if err != nil {
		return "", err
	}
	defer os.RemoveAll(temporary)
	reader, err := zip.NewReader(bytes.NewReader(archive), int64(len(archive)))
	if err != nil {
		return "", err
	}
	seen := map[string]bool{}
	for _, entry := range reader.File {
		record, ok := index[entry.Name]
		if !ok || seen[entry.Name] || !safeName(entry.Name) || !entry.Mode().IsRegular() || entry.UncompressedSize64 != uint64(record.Size) {
			return "", fmt.Errorf("unexpected payload entry: %s", entry.Name)
		}
		seen[entry.Name] = true
		output := filepath.Join(temporary, filepath.FromSlash(entry.Name))
		if err = os.MkdirAll(filepath.Dir(output), 0700); err != nil {
			return "", err
		}
		input, e := entry.Open()
		if e != nil {
			return "", e
		}
		f, e := os.OpenFile(output, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0600)
		if e != nil {
			input.Close()
			return "", e
		}
		_, e = io.Copy(f, input)
		input.Close()
		closeErr := f.Close()
		if e != nil {
			return "", e
		}
		if closeErr != nil {
			return "", closeErr
		}
	}
	if len(seen) != len(index) {
		return "", fmt.Errorf("incomplete embedded payload")
	}
	if err = verify(temporary, index); err != nil {
		return "", err
	}
	if err = os.Rename(temporary, destination); err != nil {
		// Another instance may have atomically published the same payload.
		if check := verify(destination, index); check != nil {
			return "", err
		}
	}
	return destination, nil
}

func run() error {
	cache, err := os.UserCacheDir()
	if err != nil {
		return err
	}
	base := filepath.Join(cache, "SAIO", "portable")
	var index map[string]fileRecord
	if err = json.Unmarshal(indexJSON, &index); err != nil {
		return err
	}
	runtime, err := prepare(base, payload, index)
	if err != nil {
		return err
	}
	config, err := os.UserConfigDir()
	if err != nil {
		return err
	}
	data := filepath.Join(config, "SimLab")
	if err = os.MkdirAll(data, 0700); err != nil {
		return err
	}
	if len(os.Args) == 2 && (os.Args[1] == "--extract-only") {
		fmt.Println(runtime)
		return nil
	}
	cmd := exec.Command(filepath.Join(runtime, "SAIO.exe"), os.Args[1:]...)
	cmd.Dir = data
	cmd.Env = portableEnvironment(os.Environ(), data)
	cmd.Stdout, cmd.Stderr = os.Stdout, os.Stderr
	if err = cmd.Start(); err != nil {
		return err
	}
	if len(os.Args) > 1 {
		return cmd.Wait()
	}
	return cmd.Process.Release()
}

func main() {
	if err := run(); err != nil {
		if len(os.Args) > 1 {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(1)
		}
		message := "Simulator All-in-One portable could not start.\n\n" + err.Error() + "\n\nCheck available disk space and the SAIO cache folder."
		if cache, e := os.UserCacheDir(); e == nil {
			directory := filepath.Join(cache, "SAIO")
			_ = os.MkdirAll(directory, 0700)
			_ = os.WriteFile(filepath.Join(directory, "portable-error.log"), []byte(message+"\n"), 0600)
		}
		reportError(message)
		os.Exit(1)
	}
}

// Keep runtime cache separate from user projects and replace stale launch overrides.
func portableEnvironment(original []string, data string) []string {
	result := make([]string, 0, len(original)+1)
	for _, entry := range original {
		key, _, _ := strings.Cut(entry, "=")
		if strings.EqualFold(key, "SIMLAB_DATA_DIR") {
			continue
		}
		result = append(result, entry)
	}
	return append(result, "SIMLAB_DATA_DIR="+data)
}
