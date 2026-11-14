//! Adds one LC_LOAD_DYLIB load command to a thin 64-bit Mach-O executable, using the
//! padding between the load commands and the first section (no other bytes move).

const MH_MAGIC_64: u32 = 0xfeed_facf;
const LC_LOAD_DYLIB: u32 = 0xc;
const LC_LOAD_WEAK_DYLIB: u32 = 0x8000_0018;
const LC_SEGMENT_64: u32 = 0x19;
const LC_CODE_SIGNATURE: u32 = 0x1d;

fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

pub fn add_load_dylib(bytes: &mut Vec<u8>, name: &str) -> Result<bool, String> {
    if bytes.len() < 32 || u32_at(bytes, 0) != MH_MAGIC_64 {
        return Err("not a thin 64-bit Mach-O program".into());
    }
    let ncmds = u32_at(bytes, 16) as usize;
    let sizeofcmds = u32_at(bytes, 20) as usize;
    let mut off = 32usize;
    let mut first_section = usize::MAX;
    for _ in 0..ncmds {
        let cmd = u32_at(bytes, off);
        let size = u32_at(bytes, off + 4) as usize;
        match cmd {
            LC_CODE_SIGNATURE => return Err("the program is still signed; remove the signature first".into()),
            LC_LOAD_DYLIB | LC_LOAD_WEAK_DYLIB => {
                let name_off = u32_at(bytes, off + 8) as usize;
                let raw = &bytes[off + name_off..off + size];
                let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
                if &raw[..end] == name.as_bytes() {
                    return Ok(false); // already there
                }
            }
            LC_SEGMENT_64 => {
                let nsects = u32_at(bytes, off + 64) as usize;
                for s in 0..nsects {
                    let sect = off + 72 + s * 80;
                    let file_off = u32_at(bytes, sect + 48) as usize;
                    let flags = u32_at(bytes, sect + 64) & 0xff;
                    let zerofill = matches!(flags, 0x1 | 0xc | 0x12);
                    if file_off > 0 && !zerofill {
                        first_section = first_section.min(file_off);
                    }
                }
            }
            _ => {}
        }
        off += size;
    }
    let end = 32 + sizeofcmds;
    let cmdsize = (24 + name.len() + 1 + 7) & !7;
    if first_section == usize::MAX || end + cmdsize > first_section {
        return Err("no room in the program's header for one more library".into());
    }
    if bytes[end..end + cmdsize].iter().any(|&b| b != 0) {
        return Err("the header padding is not empty".into());
    }
    let mut cmd = Vec::with_capacity(cmdsize);
    for v in [LC_LOAD_DYLIB, cmdsize as u32, 24, 2, 0, 0] {
        cmd.extend_from_slice(&v.to_le_bytes());
    }
    cmd.extend_from_slice(name.as_bytes());
    cmd.resize(cmdsize, 0);
    bytes[end..end + cmdsize].copy_from_slice(&cmd);
    bytes[16..20].copy_from_slice(&((ncmds + 1) as u32).to_le_bytes());
    bytes[20..24].copy_from_slice(&((sizeofcmds + cmdsize) as u32).to_le_bytes());
    Ok(true)
}
