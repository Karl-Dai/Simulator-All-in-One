use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs8::{EncodePrivateKey, LineEnding};

/// 规范化用户提供的证书/密钥文件路径:去掉首尾空白,并剥掉**一对**包裹引号。
///
/// 动机:Windows 资源管理器「复制为路径 / Copy as path」会自动给路径套上双引号
/// (`"C:\...\ca.crt"`),而 `"` 是 Windows 非法文件名字符,带引号直接交给
/// `std::fs::read` 会得到 `os error 123` (ERROR_INVALID_NAME)。从配置文件或手工
/// 粘贴而来的路径也常带尾随空白/换行。这里统一清洗,使这类路径开箱即用。
///
/// 仅剥掉**成对**的首尾引号,内部空格(如 `C:\my certs\ca.crt`)原样保留 ——
/// 加引号本就是为了容纳空格。返回输入的子切片,不额外分配。
pub fn sanitize_fs_path(raw: &str) -> &str {
    let trimmed = raw.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        // `"` / `'` 均为单字节 ASCII,首尾切片落在字符边界上,安全。
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return trimmed[1..trimmed.len() - 1].trim();
        }
    }
    trimmed
}

/// 读取客户端/服务端私钥 PEM 文件,统一规范化为 PKCS#8 PEM 字节,
/// 以便交给 native-tls 的 `Identity::from_pkcs8`。
///
/// - `-----BEGIN PRIVATE KEY-----` (PKCS#8): 原样返回
/// - `-----BEGIN RSA PRIVATE KEY-----` (PKCS#1): 解码后重新编码为 PKCS#8
/// - 其它(EC/加密/未知): 返回明确错误
pub fn load_key_as_pkcs8_pem(path: &str) -> Result<Vec<u8>, String> {
    let path = sanitize_fs_path(path);
    let pem = std::fs::read_to_string(path)
        .map_err(|e| format!("读取密钥失败 {}: {}", path, e))?;

    if pem.contains("-----BEGIN PRIVATE KEY-----") {
        return Ok(pem.into_bytes());
    }

    if pem.contains("-----BEGIN RSA PRIVATE KEY-----") {
        let rsa_key = rsa::RsaPrivateKey::from_pkcs1_pem(&pem)
            .map_err(|e| format!("解析 PKCS#1 RSA 私钥失败 {}: {}", path, e))?;
        let pkcs8 = rsa_key
            .to_pkcs8_pem(LineEnding::LF)
            .map_err(|e| format!("转换 PKCS#1 → PKCS#8 失败 {}: {}", path, e))?;
        return Ok(pkcs8.as_bytes().to_vec());
    }

    if pem.contains("-----BEGIN ENCRYPTED PRIVATE KEY-----") {
        return Err(format!("不支持加密的私钥 {}: 请先解密为明文 PKCS#8/PKCS#1 PEM", path));
    }

    if pem.contains("-----BEGIN EC PRIVATE KEY-----") {
        return Err(format!(
            "不支持 SEC1 EC 私钥 {}: 请用 `openssl pkcs8 -topk8 -nocrypt -in <key>` 转为 PKCS#8 PEM",
            path
        ));
    }

    Err(format!("无法识别的私钥格式 {}: 仅支持 PKCS#1 / PKCS#8 PEM", path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_tmp(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    fn gen_pkcs1_pem() -> String {
        use rsa::pkcs1::EncodeRsaPrivateKey;
        let mut rng = rand::rngs::OsRng;
        let key = rsa::RsaPrivateKey::new(&mut rng, 2048).unwrap();
        key.to_pkcs1_pem(LineEnding::LF).unwrap().to_string()
    }

    fn gen_pkcs8_pem() -> String {
        let mut rng = rand::rngs::OsRng;
        let key = rsa::RsaPrivateKey::new(&mut rng, 2048).unwrap();
        key.to_pkcs8_pem(LineEnding::LF).unwrap().to_string()
    }

    #[test]
    fn pkcs1_is_converted_to_pkcs8() {
        let pkcs1 = gen_pkcs1_pem();
        assert!(pkcs1.starts_with("-----BEGIN RSA PRIVATE KEY-----"));
        let f = write_tmp(&pkcs1);
        let out = load_key_as_pkcs8_pem(f.path().to_str().unwrap()).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.starts_with("-----BEGIN PRIVATE KEY-----"), "got: {}", s);
        // 应能被 PKCS#8 解码器重新解析
        use rsa::pkcs8::DecodePrivateKey;
        rsa::RsaPrivateKey::from_pkcs8_pem(&s).unwrap();
    }

    #[test]
    fn pkcs8_is_passed_through() {
        let pkcs8 = gen_pkcs8_pem();
        let f = write_tmp(&pkcs8);
        let out = load_key_as_pkcs8_pem(f.path().to_str().unwrap()).unwrap();
        assert_eq!(out, pkcs8.as_bytes());
    }

    #[test]
    fn quoted_path_is_accepted() {
        // 模拟 Windows「复制为路径 / Copy as path」粘贴进来的带引号路径
        // (`"..."`)。`"` 是 Windows 非法文件名字符,未清洗直接读取会得到
        // os error 123。清洗后应正常读取。
        let pkcs8 = gen_pkcs8_pem();
        let f = write_tmp(&pkcs8);
        let quoted = format!("\"{}\"", f.path().to_str().unwrap());
        let out = load_key_as_pkcs8_pem(&quoted).unwrap();
        assert_eq!(out, pkcs8.as_bytes());
    }

    #[test]
    fn whitespace_wrapped_path_is_accepted() {
        // 配置文件/粘贴常见的首尾空白与尾随换行。
        let pkcs8 = gen_pkcs8_pem();
        let f = write_tmp(&pkcs8);
        let padded = format!("  {}\r\n", f.path().to_str().unwrap());
        let out = load_key_as_pkcs8_pem(&padded).unwrap();
        assert_eq!(out, pkcs8.as_bytes());
    }

    #[test]
    fn missing_file_returns_error_with_path() {
        let err = load_key_as_pkcs8_pem("/definitely/not/a/real/path.key").unwrap_err();
        assert!(err.contains("/definitely/not/a/real/path.key"));
    }

    #[test]
    fn sanitize_strips_double_quotes() {
        assert_eq!(sanitize_fs_path("\"C:\\certs\\ca.crt\""), "C:\\certs\\ca.crt");
    }

    #[test]
    fn sanitize_strips_single_quotes() {
        assert_eq!(sanitize_fs_path("'/etc/ssl/ca.pem'"), "/etc/ssl/ca.pem");
    }

    #[test]
    fn sanitize_trims_whitespace_and_newline() {
        assert_eq!(sanitize_fs_path("  /etc/ssl/ca.pem\r\n"), "/etc/ssl/ca.pem");
        // 引号外侧 + 内侧的空白都应被清掉
        assert_eq!(sanitize_fs_path(" \" /etc/ssl/ca.pem \" "), "/etc/ssl/ca.pem");
    }

    #[test]
    fn sanitize_preserves_internal_spaces() {
        // 路径内部空格(加引号的本意)必须保留
        assert_eq!(sanitize_fs_path("\"C:\\my certs\\ca.crt\""), "C:\\my certs\\ca.crt");
    }

    #[test]
    fn sanitize_leaves_plain_path_untouched() {
        assert_eq!(sanitize_fs_path("/etc/ssl/ca.pem"), "/etc/ssl/ca.pem");
    }

    #[test]
    fn sanitize_keeps_unmatched_or_lone_quote() {
        // 仅单侧引号、或路径内含合法引号(非 Windows),不应被剥
        assert_eq!(sanitize_fs_path("\"/etc/ssl/ca.pem"), "\"/etc/ssl/ca.pem");
        assert_eq!(sanitize_fs_path("/etc/ssl/ca\".pem"), "/etc/ssl/ca\".pem");
        assert_eq!(sanitize_fs_path("\""), "\"");
        assert_eq!(sanitize_fs_path(""), "");
    }

    #[test]
    fn unknown_format_is_rejected() {
        let f = write_tmp("-----BEGIN CERTIFICATE-----\nMIIB...\n-----END CERTIFICATE-----\n");
        let err = load_key_as_pkcs8_pem(f.path().to_str().unwrap()).unwrap_err();
        assert!(err.contains("无法识别的私钥格式"));
    }
}
