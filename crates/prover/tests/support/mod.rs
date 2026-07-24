use std::path::PathBuf;

pub const KEY_THUMBPRINT: [u8; 32] = [0x51; 32];
pub const SNAPSHOT_SALT: [u8; 32] = [0x62; 32];

pub fn guest_elf_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../target/elf-compilation/riscv64im-succinct-zkvm-elf/release/raziel-authorization-guest",
    )
}
