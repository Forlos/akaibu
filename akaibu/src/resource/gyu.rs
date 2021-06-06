use super::{ResourceScheme, ResourceType};
use crate::{
    error::AkaibuError,
    util::{image::bitmap_to_png_with_padding, mt::Mt19937},
};
use anyhow::Context;
use image::{buffer::ConvertBuffer, ImageBuffer};
use once_cell::sync::Lazy;
use scroll::{Pread, BE, LE};
use std::{collections::HashMap, fs::File, io::Read, path::Path};

const SEEDS_PATH: &str = "gyu/seeds.json";

static SEEDS_TABLE: Lazy<HashMap<String, Vec<u32>>> = Lazy::new(|| {
    let seeds_table: HashMap<String, Vec<u32>> = serde_json::from_slice(
        &crate::Resources::get(SEEDS_PATH)
            .expect("Could not find file: gyu/seeds.json"),
    )
    .expect("Could not deserialize resource json");
    seeds_table
});

#[derive(Debug, Pread)]
struct GyuHeader {
    magic: [u8; 4],
    version: u32,
    mt_seed: u32,
    bpp: u32,
    width: u32,
    height: u32,
    data_size: u32,
    alpha_channel_size: u32,
    color_table_size: u32,
}

#[derive(Debug, Clone)]
pub(crate) enum GyuScheme {
    DemonBusters,
    HakoniwaLogic,
    HoshizoraTeaParty,
    Imopara1,
    Imopara1Jpn,
    Imopara2,
    Imopara3,
    KagiTori,
    KaraNoShoujo,
    KonekoNekoNeko,
    LoveLoveLife,
    Ojousama,
    OpenWorld,
    TsukiNoShoujo,
    UchiNoImouto,
    UchiNoKoibito,
    Yuuwaku,
    WanNyan,
    NyanCafe,
    Universal,
}

impl ResourceScheme for GyuScheme {
    fn convert(&self, file_path: &Path) -> anyhow::Result<ResourceType> {
        let mut buf = Vec::with_capacity(1 << 20);
        let mut file = File::open(file_path)?;
        file.read_to_end(&mut buf)?;
        self.from_bytes(buf, file_path)
    }
    fn convert_from_bytes(
        &self,
        file_path: &Path,
        buf: Vec<u8>,
    ) -> anyhow::Result<ResourceType> {
        self.from_bytes(buf, file_path)
    }
    fn get_name(&self) -> String {
        format!("[GYU] {}",match self {
            Self::DemonBusters => "Demon Busters ~Ecchi na Ecchi na Demon Taiji~",
            Self::HakoniwaLogic => "Hakoniwa Logic",
            Self::HoshizoraTeaParty => "Hoshizora Tea Party",
            Self::Imopara1 => "Imouto Paradise! ~Onii-chan to Go nin no Imouto no Ecchi Shimakuri na Mainichi~ [ENG]",
            Self::Imopara1Jpn => "Imouto Paradise! ~Onii-chan to Go nin no Imouto no Ecchi Shimakuri na Mainichi~ [JPN]",
            Self::Imopara2 => "Imouto Paradise! 2 ~Onii-chan to Go nin no Imouto no Motto! Ecchi Shimakuri na Mainichi~",
            Self::Imopara3 => "Imouto Paradise! 3 ~Onii-chan to Go nin no Imouto no Sugoku! Ecchi Shimakuri na Mainichi~",
            Self::KagiTori => "Kagi o Kakushita Kago no Tori -Bird in Cage Hiding the Key-",
            Self::KaraNoShoujo => "Kara no Shoujo -Bishoujo Gakuen 1-",
            Self::KonekoNekoNeko => "Koneko Neko Neko",
            Self::LoveLoveLife =>"Love Love Life ~Ojou-sama 7nin to Love Love Harem Seikatsu~",
            Self::Ojousama => "Ojou-sama to Himitsu no Otome",
            Self::OpenWorld => "Sex Open World e Youkoso!",
            Self::TsukiNoShoujo => "Tsuki no Shoujo - Bishoujo Gakuen 2 -",
            Self::UchiNoImouto => "Uchi no Imouto",
            Self::UchiNoKoibito => "Uchi no Koibito",
            Self::Yuuwaku => "Yuuwaku Scramble",
            Self::WanNyan => "Wan Nyan ☆ A La Mode! ~Docchi ni Suru no? Wan Nyan H na Café Jijou!~",
            Self::NyanCafe => "Nyan Café Macchiato ~Neko ga Iru Café no Ecchi Jijou~",
            Self::Universal => "Universal"
        }
                )
    }
    fn get_schemes() -> Vec<Box<dyn ResourceScheme>>
    where
        Self: Sized,
    {
        vec![
            Box::new(Self::DemonBusters),
            Box::new(Self::HakoniwaLogic),
            Box::new(Self::HoshizoraTeaParty),
            Box::new(Self::Imopara1),
            Box::new(Self::Imopara1Jpn),
            Box::new(Self::Imopara2),
            Box::new(Self::Imopara3),
            Box::new(Self::KagiTori),
            Box::new(Self::KaraNoShoujo),
            Box::new(Self::KonekoNekoNeko),
            Box::new(Self::LoveLoveLife),
            Box::new(Self::Ojousama),
            Box::new(Self::OpenWorld),
            Box::new(Self::TsukiNoShoujo),
            Box::new(Self::UchiNoImouto),
            Box::new(Self::UchiNoKoibito),
            Box::new(Self::Yuuwaku),
            Box::new(Self::WanNyan),
            Box::new(Self::NyanCafe),
            Box::new(Self::Universal),
        ]
    }
}

impl GyuScheme {
    fn from_bytes(
        &self,
        mut buf: Vec<u8>,
        file_path: &Path,
    ) -> anyhow::Result<ResourceType> {
        let mut header = buf.pread::<GyuHeader>(0)?;
        if header.mt_seed == 0 {
            let seeds = self.get_seeds()?;
            let id: usize = file_path
                .file_stem()
                .context("File name not found")?
                .to_str()
                .context("Invalid string")?
                .parse()?;
            header.mt_seed = *seeds.get(id).context("Could not get mt_seed")?;
        }
        let padded_width =
            ((header.bpp / 8 * header.width + 3) & 0xFF_FF_FF_FC) as usize;
        let data_offset = 36 + (header.color_table_size as usize * 4);
        if (header.version & 0xFFFF_0000) != 0x0100_0000 {
            decrypt_with_mt(
                &mut buf[data_offset..data_offset + header.data_size as usize],
                header.mt_seed,
            );
        }
        let color_table = &buf[36..data_offset];
        let mut data = bitmap_to_png_with_padding(
            decompress(
                &buf[data_offset..data_offset + header.data_size as usize],
                padded_width * header.height as usize,
                header.version,
            )?,
            padded_width,
            padded_width - (header.bpp / 8 * header.width) as usize,
        );
        let alpha_channel = bitmap_to_png_with_padding(
            decompress0(
                &buf[data_offset + header.data_size as usize
                    ..data_offset
                        + header.data_size as usize
                        + header.alpha_channel_size as usize],
                ((header.width + 3) & 0xFF_FF_FF_FC) as usize
                    * header.height as usize,
            ),
            ((header.width + 3) & 0xFF_FF_FF_FC) as usize,
            (((header.width + 3) & 0xFF_FF_FF_FC) - header.width) as usize,
        );
        if header.bpp == 8 && header.color_table_size != 0 {
            data = resolve_color_table(&data, &color_table);
        } else if header.bpp == 24 {
            data = add_alpha_channel(data);
        }
        resolve_alpha_channel(&mut data, &alpha_channel);
        let image: ImageBuffer<image::Bgra<u8>, Vec<u8>> =
            ImageBuffer::from_vec(
                header.width as u32,
                header.height as u32,
                data,
            )
            .context("Invalid image resolution")?;
        Ok(ResourceType::RgbaImage {
            image: image.convert(),
        })
    }
    fn get_seeds(&self) -> anyhow::Result<&Vec<u32>> {
        SEEDS_TABLE
            .get(self.get_key())
            .context(format!("Unsupported game key {}", self.get_key()))
    }
    fn get_key(&self) -> &str {
        match self {
            Self::DemonBusters => "demonbusters",
            Self::HakoniwaLogic => "hakoniwalogic",
            Self::HoshizoraTeaParty => "hoshizorateaparty",
            Self::Imopara1 => "imopara1",
            Self::Imopara1Jpn => "imopara1JP",
            Self::Imopara2 => "imopara2",
            Self::Imopara3 => "imopara3",
            Self::KagiTori => "kagitori",
            Self::KaraNoShoujo => "karanoshoujo",
            Self::KonekoNekoNeko => "konekonekoneko",
            Self::LoveLoveLife => "lovelovelife",
            Self::Ojousama => "ojousama",
            Self::OpenWorld => "openworld",
            Self::TsukiNoShoujo => "tsukinoshoujo",
            Self::UchiNoImouto => "uchinoimouto",
            Self::UchiNoKoibito => "uchinokoibito",
            Self::Yuuwaku => "yuuwaku",
            Self::WanNyan => "wannyan",
            Self::NyanCafe => "nyancafe",
            Self::Universal => "universal",
        }
    }
}

fn decompress(
    src: &[u8],
    dest_len: usize,
    version: u32,
) -> anyhow::Result<Vec<u8>> {
    let version = version & 0xFFFF_0000;
    Ok(match version {
        0x0800_0000 => decompress3(&src[4..], dest_len)?,
        0x0400_0000 | 0x0200_0000 => decompress0(src, dest_len),
        0x0100_0000 => Vec::from(src),
        _ => {
            return Err(AkaibuError::Custom(format!(
                "Version not supported {}",
                version
            ))
            .into())
        }
    })
}

fn decompress3(src: &[u8], dest_len: usize) -> anyhow::Result<Vec<u8>> {
    let mut dest = vec![0u8; dest_len];
    let mut src_index = 0;
    let mut di = 0;
    let mut c;
    let mut d = 1;
    let mut dl = 0;
    let mut read_first = true;

    loop {
        if read_first {
            *dest.get_mut(di).context("Out of bounds write")? =
                *src.get(src_index).context("Out of bounds read")?;
            di += 1;
            src_index += 1;
            read_first = false
        }

        d -= 1;
        if d == 0 {
            dl = *src.get(src_index).context("Out of bounds read")?;
            src_index += 1;
            d = 8;
        }

        let (temp, overflow) = dl.overflowing_add(dl);
        dl = temp;
        if overflow {
            read_first = true;
            continue;
        }

        let mut a = [0xFF; 4];
        d -= 1;
        if d == 0 {
            dl = *src.get(src_index).context("Out of bounds read")?;
            src_index += 1;
            d = 8;
        }

        let (temp, overflow) = dl.overflowing_add(dl);
        dl = temp;
        if overflow {
            c = src.pread_with::<u16>(src_index, BE)?;
            a[0] = c as u8;
            a[1] = (c >> 8) as u8;
            src_index += 2;
            let mut temp_a = a.pread_with::<u32>(0, LE)?;
            c = temp_a as u16 & 7;
            temp_a >>= 3;
            temp_a |= 0xFF_FF_E0_00;
            a = temp_a.to_le_bytes();
            if c == 0 {
                let cl = src[src_index];
                c |= cl as u16;
                src_index += 1;
                if cl == 0 {
                    return Ok(dest);
                }
            } else {
                c += 1;
            }
        } else {
            c = 0;
            d -= 1;
            if d == 0 {
                dl = *src.get(src_index).context("Out of bounds read")?;
                src_index += 1;
                d = 8;
            }
            let (temp, overflow) = dl.overflowing_add(dl);
            dl = temp;
            c += c;
            if overflow {
                c += 1;
            }
            d -= 1;
            if d == 0 {
                dl = *src.get(src_index).context("Out of bounds read")?;
                src_index += 1;
                d = 8;
            }
            let (temp, overflow) = dl.overflowing_add(dl);
            dl = temp;
            c += c;
            if overflow {
                c += 1;
            }
            a[0] = *src.get(src_index).context("Out of bounds read")?;
            src_index += 1;
            c += 1;
        }

        let mut si = di as i32;
        si += a.pread_with::<i32>(0, LE)?;
        c += 1;
        for _ in 0..c {
            *dest.get_mut(di).context("Out of bounds write")? =
                dest[si as usize];
            di += 1;
            si += 1;
        }
    }
}

fn decompress0(buf: &[u8], dest_len: usize) -> Vec<u8> {
    if buf.is_empty() {
        return vec![];
    }
    let mut dest = vec![0u8; dest_len];
    let mut lookup_table = vec![0u8; 4096];

    let mut x = 0_u16;
    let mut lookup_index = 4078;
    let mut bytes_read = 0;
    let mut bytes_written = 0;
    while bytes_read < buf.len() {
        x >>= 1;
        if (x & 0x100) == 0 {
            x = buf[bytes_read] as u16;
            bytes_read += 1;
            x |= 0xFF00;
        }
        if ((x & 0xFF) & 1) == 0 {
            let bl = buf[bytes_read];
            bytes_read += 1;
            let cl = buf[bytes_read];
            bytes_read += 1;
            let mut s = cl as u16;
            let mut d = s as u16;
            let mut c = bl as u16;
            d &= 0xF0;
            s &= 0x0F;
            d <<= 4;
            s += 3;
            d |= c;
            c = s;
            if c > 0 {
                s = d;
                let mut counter = c;
                while counter != 0 {
                    c = s;
                    s += 1;
                    c &= 0xFFF;
                    d = lookup_table[c as usize] as u16;
                    dest[bytes_written] = d as u8;
                    c = lookup_index;
                    bytes_written += 1;
                    lookup_index += 1;
                    lookup_index &= 0xFFF;
                    lookup_table[c as usize] = d as u8;

                    counter -= 1;
                }
            }
        } else {
            let d = buf[bytes_read];
            bytes_read += 1;
            dest[bytes_written] = d;
            bytes_written += 1;
            let c = lookup_index;
            lookup_index += 1;
            lookup_index &= 0xFFF;
            lookup_table[c as usize] = d;
        }
    }
    dest
}

fn decrypt_with_mt(buf: &mut [u8], mt_seed: u32) {
    if mt_seed != 0xFFFF_FFFF {
        let mut mt = Mt19937::default();
        mt.seed_gyu(mt_seed);
        for _ in 0..10 {
            let a = mt.gen_u32() % buf.len() as u32;
            let b = mt.gen_u32() % buf.len() as u32;
            buf.swap(a as usize, b as usize);
        }
    }
}

fn resolve_color_table(buf: &[u8], color_table: &[u8]) -> Vec<u8> {
    buf.iter()
        .fold(Vec::with_capacity(buf.len() * 4), |mut v, b| {
            v.extend_from_slice(
                &color_table[*b as usize * 4..*b as usize * 4 + 4],
            );
            v
        })
}

fn add_alpha_channel(buf: Vec<u8>) -> Vec<u8> {
    buf.chunks_exact(3)
        .map(|c| {
            let mut c = c.to_vec();
            c.push(0xFF);
            c
        })
        .flatten()
        .collect()
}

fn resolve_alpha_channel(buf: &mut [u8], alpha_channel: &[u8]) {
    if !alpha_channel.is_empty() {
        buf.chunks_exact_mut(4)
            .enumerate()
            .for_each(|(i, c)| c[3] = alpha_channel[i]);
    } else {
        buf.chunks_exact_mut(4).for_each(|c| c[3] = 0xFF);
    }
}
