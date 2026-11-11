# Storage validation

Run from repository root:

- `SAIO_CROSS_LANGUAGE=1 go test ./storage/go -v` builds the Rust harness and round-trips the same actual SQLite file through Go and Rust. Includes Unicode keys/text, empty objects/arrays, null, booleans, signed/unsigned 64-bit boundaries, finite floats, compare-and-swap conflicts, migration safety, concurrent initialization and project role validation.
- `cargo test --manifest-path storage/harness/Cargo.toml` checks Rust migration, stale writers, typed values and selected-document project export/import.
- `go test ./internal/suite ./ui` checks launcher one-time migration, stale launcher writers, failed-restore write blocking, preference reload and appearance contracts.
- `CGO_ENABLED=0 GOOS=windows GOARCH=amd64 go test ./storage/go -c -o .validation/unified-storage-go-windows.test.exe` cross-compiles the pure-Go SQLite driver; it does not execute Windows code.
- `SAIO_SIZE_BENCH=1 go test ./storage/go -run TestActualTreeSize -v` measures the deployed typed-node schema (not a substitute point table). Values are a representative six-field point list with names and finite values. Database size is measured after WAL checkpoint.

Measured 2026-11-11 on macOS:

| Points | Compact JSON | Pretty JSON | SQLite typed records |
| ---: | ---: | ---: | ---: |
| 1,000 | 85,498 B | 143,506 B | 90,112 B |
| 10,000 | 884,700 B | 1,464,708 B | 679,936 B |
| 100,000 | 9,146,702 B | 14,946,710 B | 6,881,280 B |

This typed tree packs each container’s direct scalar fields as a compact typed record with interned field IDs, while nested containers remain stable SQL nodes. It contains no stored JSON and rewrites only changed container records. For 10k and 100k point samples it is smaller than compact JSON; small files retain fixed SQLite schema/page overhead. These results do not use the earlier unrelated point-column prototype.

The automated engine tests above use temporary directories. Native suite verification and the repaired accidental PMU launch are recorded separately in `docs/unified-storage-validation-2026-11-11.md`.
