use windows::Win32::UI::Shell::IsUserAnAdmin;

/// Whether the current process token is a member of the Administrators group
/// AND running elevated. Returns false for non-admin users and for admin users
/// in a non-elevated session (UAC).
pub fn is_admin() -> bool {
    // SAFETY: IsUserAnAdmin is a no-argument Win32 call that returns BOOL.
    unsafe { IsUserAnAdmin().as_bool() }
}
