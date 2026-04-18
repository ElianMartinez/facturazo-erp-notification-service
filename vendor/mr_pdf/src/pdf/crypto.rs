pub struct Rc4 {
    i: u8,
    j: u8,
    s: [u8; 256],
}

impl Rc4 {
    pub fn new(key: &[u8]) -> Self {
        let mut s = [0u8; 256];
        for i in 0..=255 {
            s[i as usize] = i;
        }
        let mut j: u8 = 0;
        if !key.is_empty() {
            for i in 0..=255 {
                j = j
                    .wrapping_add(s[i as usize])
                    .wrapping_add(key[i as usize % key.len()]);
                s.swap(i as usize, j as usize);
            }
        }
        Self { i: 0, j: 0, s }
    }

    pub fn apply_keystream(&mut self, data: &mut [u8]) {
        for b in data.iter_mut() {
            self.i = self.i.wrapping_add(1);
            self.j = self.j.wrapping_add(self.s[self.i as usize]);
            self.s.swap(self.i as usize, self.j as usize);
            let k = self.s[self.i as usize].wrapping_add(self.s[self.j as usize]);
            *b ^= self.s[k as usize];
        }
    }
}

// PDF 1.4 Standard Security Handler Padding
const PADDING: [u8; 32] = [
    0x28, 0xbf, 0x4e, 0x5e, 0x4e, 0x75, 0x8a, 0x41, 0x64, 0x00, 0x4e, 0x56, 0xff, 0xfa, 0x01, 0x08,
    0x2e, 0x2e, 0x00, 0xb6, 0xd0, 0x68, 0x3e, 0x80, 0x2f, 0x0c, 0xa9, 0xfe, 0x64, 0x53, 0x69, 0x7a,
];

#[derive(Clone, Copy, Debug)]
pub struct PdfPermissions {
    pub can_print: bool,
    pub can_modify: bool,
    pub can_copy: bool,
    pub can_add_notes: bool,
}

impl Default for PdfPermissions {
    fn default() -> Self {
        Self {
            can_print: true,
            can_modify: true,
            can_copy: true,
            can_add_notes: true,
        }
    }
}

pub struct SecurityHandler {
    pub o: [u8; 32],
    pub u: [u8; 32],
    pub p: i32,
    pub encryption_key: Vec<u8>,
}

impl SecurityHandler {
    fn pad_pwd(pwd: &str) -> [u8; 32] {
        let mut padded = [0u8; 32];
        let bytes = pwd.as_bytes();
        let len = bytes.len().min(32);
        padded[..len].copy_from_slice(&bytes[..len]);
        if len < 32 {
            padded[len..].copy_from_slice(&PADDING[..32 - len]);
        }
        padded
    }

    pub fn new(owner: &str, user: &str, perms: PdfPermissions, doc_id: &[u8]) -> Self {
        let mut p: u32 = 0xFFFFFFFC;
        if !perms.can_print {
            p &= !(1 << 2);
        }
        if !perms.can_modify {
            p &= !(1 << 3);
        }
        if !perms.can_copy {
            p &= !(1 << 4);
        }
        if !perms.can_add_notes {
            p &= !(1 << 5);
        }
        let p_i32 = p as i32;

        let user_pad = Self::pad_pwd(user);
        let owner_pad = if owner.is_empty() {
            user_pad
        } else {
            Self::pad_pwd(owner)
        };

        // Algorithm 3.3 computing O (Rev 3) - Uses OWNER password to encrypt USER password
        let mut o_key = md5::compute(&owner_pad).0.to_vec();
        for _ in 0..50 {
            o_key = md5::compute(&o_key).0.to_vec();
        }

        let mut o = user_pad; // encrypt the USER padded password
        let mut rc4_key = [0u8; 16];
        rc4_key.copy_from_slice(&o_key[..16]);

        for i in 0..=19u8 {
            let mut current_key = rc4_key.clone();
            for j in 0..16 {
                current_key[j] ^= i;
            }
            let mut rc4 = Rc4::new(&current_key);
            rc4.apply_keystream(&mut o);
        }

        // Algorithm 3.2 computing Encryption Key
        let mut ctx = md5::Context::new();
        ctx.consume(&user_pad);
        ctx.consume(&o);
        ctx.consume(&p_i32.to_le_bytes());
        ctx.consume(doc_id); // Document ID
        let mut enc_key = ctx.finalize().0.to_vec();
        for _ in 0..50 {
            enc_key = md5::compute(&enc_key[..16]).0.to_vec();
        }
        let encryption_key = enc_key[..16].to_vec(); // 128-bit key (Revision 3)

        // Algorithm 3.5 computing U (Rev 3)
        let mut ctx = md5::Context::new();
        ctx.consume(&PADDING);
        ctx.consume(doc_id);
        let mut u_res = ctx.finalize().0.to_vec();

        for i in 0..=19u8 {
            let mut current_key = encryption_key.clone();
            for j in 0..16 {
                current_key[j] ^= i;
            }
            let mut rc4 = Rc4::new(&current_key);
            rc4.apply_keystream(&mut u_res);
        }

        let mut u = [0u8; 32];
        u[..16].copy_from_slice(&u_res[..16]);
        u[16..].copy_from_slice(&PADDING[..16]);

        Self {
            o,
            u,
            p: p_i32,
            encryption_key,
        }
    }

    pub fn get_obj_key(&self, obj_num: u32, gen_num: u16) -> Vec<u8> {
        let mut ctx = md5::Context::new();
        ctx.consume(&self.encryption_key);
        let obj_bytes = obj_num.to_le_bytes();
        ctx.consume(&obj_bytes[0..3]);
        let gen_bytes = gen_num.to_le_bytes();
        ctx.consume(&gen_bytes[0..2]);
        let hash = ctx.finalize().0;
        let len = (self.encryption_key.len() + 5).min(16);
        hash[..len].to_vec()
    }

    pub fn encrypt_bytes(&self, obj_num: u32, gen_num: u16, data: &mut [u8]) {
        let key = self.get_obj_key(obj_num, gen_num);
        let mut rc4 = Rc4::new(&key);
        rc4.apply_keystream(data);
    }
}
