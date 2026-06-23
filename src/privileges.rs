#[cfg(windows)]
pub fn is_admin() -> bool {
    is_elevated::is_elevated()
}

#[cfg(unix)]
pub fn is_admin() -> bool {
    unsafe { libc::getuid() == 0 }
}

#[cfg(not(any(windows, unix)))]
pub fn is_admin() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_admin() {
        let _ = is_admin();
    }
}
