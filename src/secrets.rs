use crate::storage::{data_dir, SECRET_FILE};
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

pub fn secret_path() -> PathBuf {
    data_dir().join(SECRET_FILE)
}

#[cfg(windows)]
pub fn protect(plain: &[u8]) -> Result<Vec<u8>> {
    win::crypt(plain, true)
}

#[cfg(windows)]
pub fn unprotect(cipher: &[u8]) -> Result<Vec<u8>> {
    win::crypt(cipher, false)
}

#[cfg(not(windows))]
pub fn protect(plain: &[u8]) -> Result<Vec<u8>> {
    Ok(plain.to_vec())
}

#[cfg(not(windows))]
pub fn unprotect(cipher: &[u8]) -> Result<Vec<u8>> {
    Ok(cipher.to_vec())
}

pub fn save_api_key(key: &str) -> Result<()> {
    let path = secret_path();
    if key.trim().is_empty() {
        if path.exists() {
            fs::remove_file(&path)?;
        }
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, protect(key.trim().as_bytes())?)?;
    Ok(())
}

pub fn load_api_key() -> Option<String> {
    let bytes = fs::read(secret_path()).ok()?;
    let plain = unprotect(&bytes).ok()?;
    let key = String::from_utf8(plain).ok()?;
    let key = key.trim().to_string();
    (!key.is_empty()).then_some(key)
}

#[cfg(windows)]
mod win {
    use anyhow::{bail, Result};
    use std::ffi::c_void;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    pub fn crypt(data: &[u8], encrypt: bool) -> Result<Vec<u8>> {
        let mut input = data.to_vec();
        let blob_in = CRYPT_INTEGER_BLOB {
            cbData: input.len() as u32,
            pbData: input.as_mut_ptr(),
        };
        let mut blob_out = CRYPT_INTEGER_BLOB::default();

        let ok = unsafe {
            if encrypt {
                CryptProtectData(
                    &blob_in,
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut blob_out,
                )
            } else {
                CryptUnprotectData(
                    &blob_in,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut blob_out,
                )
            }
        };

        if ok == 0 {
            bail!("DPAPI falhou: {}", std::io::Error::last_os_error());
        }

        let result = if blob_out.pbData.is_null() {
            Vec::new()
        } else {
            unsafe {
                std::slice::from_raw_parts(blob_out.pbData, blob_out.cbData as usize).to_vec()
            }
        };
        unsafe {
            LocalFree(blob_out.pbData as *mut c_void);
        }
        Ok(result)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn dpapi_roundtrip() {
        let plain = b"chave-secreta-da-mistral";
        let cipher = protect(plain).unwrap();
        assert_ne!(cipher, plain);
        let back = unprotect(&cipher).unwrap();
        assert_eq!(back, plain);
    }

    #[test]
    fn unprotect_rejects_garbage() {
        assert!(unprotect(b"nao-e-dpapi").is_err());
    }
}
