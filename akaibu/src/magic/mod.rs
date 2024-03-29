use crate::scheme::{self, Scheme};
use enum_iterator::IntoEnumIterator;

#[derive(Debug, IntoEnumIterator)]
pub enum Archive {
    Acv1,
    Cpz7,
    Gxp,
    Pf8,
    Ypf,
    Buriko,
    EscArc2,
    Malie,
    Silky,
    Iar,
    WillplusArc,
    QliePack,
    Nekopack,
    AmusePac,
    TacticsArc,
    Link6,
    NotRecognized,
}

impl Archive {
    /// Parse first few bytes of file to detect archive type
    pub fn parse(buf: &[u8]) -> Self {
        match buf {
            // ACV1
            [0x41, 0x43, 0x56, 0x31, ..] => Self::Acv1,
            // CPZ7
            [0x43, 0x50, 0x5A, 0x37, ..] => Self::Cpz7,
            // GXP\x00
            [0x47, 0x58, 0x50, 0x00, ..] => Self::Gxp,
            // pf8
            [0x70, 0x66, 0x38, ..] => Self::Pf8,
            // YFP\x00
            [0x59, 0x50, 0x46, 0x00, ..] => Self::Ypf,
            // BURIKO ARC20
            [0x42, 0x55, 0x52, 0x49, 0x4b, 0x4f, 0x20, 0x41, 0x52, 0x43, 0x32, 0x30, ..] => {
                Self::Buriko
            }
            // ESC-ARC2
            [0x45, 0x53, 0x43, 0x2D, 0x41, 0x52, 0x43, 0x32, ..] => {
                Self::EscArc2
            }
            // No magic but each game has only one archive so we can just hardcode first 4 bytes here
            [0xc1, 0xf2, 0x5e, 0x79, ..] | [0x7f, 0x4d, 0x8f, 0xe9, ..] => {
                Self::Malie
            }
            // iar
            [0x69, 0x61, 0x72, 0x20, ..] => Self::Iar,
            // NEKOPACK
            [0x4e, 0x45, 0x4b, 0x4f, 0x50, 0x41, 0x43, 0x4b, ..] => {
                Self::Nekopack
            }
            [0x50, 0x41, 0x43, 0x20, ..] => Self::AmusePac,
            // TACTICS_ARC_FILE
            [0x54, 0x41, 0x43, 0x54, 0x49, 0x43, 0x53, 0x5F, 0x41, 0x52, 0x43, 0x5F, 0x46, 0x49, 0x4C, 0x45, ..] => {
                Self::TacticsArc
            }
            // LINK6\x00\x00
            [0x4C, 0x49, 0x4E, 0x4B, 0x36, 0x00, 0x00, ..] => Self::Link6,
            _ => Self::NotRecognized,
        }
    }
    /// Parse last 32 bytes of file to detect archive type
    pub fn parse_end(buf: &[u8]) -> Self {
        if &buf[buf.len() - 0x1C..buf.len() - 0x1C + 11] == b"FilePackVer" {
            Self::QliePack
        } else {
            Self::NotRecognized
        }
    }
    /// Is archive extraction scheme not game dependent
    pub fn is_universal(&self) -> bool {
        match self {
            Self::Acv1 => false,
            Self::Cpz7 => false,
            Self::Gxp => true,
            Self::Pf8 => true,
            Self::Ypf => true,
            Self::Buriko => true,
            Self::EscArc2 => true,
            Self::Malie => false,
            Self::Silky => true,
            Self::Iar => true,
            Self::WillplusArc => true,
            Self::QliePack => false,
            Self::Nekopack => true,
            Self::AmusePac => true,
            Self::TacticsArc => false,
            Self::Link6 => true,
            Self::NotRecognized => false,
        }
    }
    /// Get list of all schemes for given archive type
    pub fn get_schemes(&self) -> Vec<Box<dyn Scheme>> {
        match self {
            Self::Acv1 => scheme::acv1::Acv1Scheme::get_schemes(),
            Self::Cpz7 => scheme::cpz7::Cpz7Scheme::get_schemes(),
            Self::Gxp => scheme::gxp::GxpScheme::get_schemes(),
            Self::Pf8 => scheme::pf8::Pf8Scheme::get_schemes(),
            Self::Ypf => scheme::ypf::YpfScheme::get_schemes(),
            Self::Buriko => scheme::buriko::BurikoScheme::get_schemes(),
            Self::EscArc2 => scheme::esc_arc2::EscArc2Scheme::get_schemes(),
            Self::Malie => scheme::malie::MalieScheme::get_schemes(),
            Self::Silky => scheme::silky::SilkyScheme::get_schemes(),
            Self::Iar => scheme::iar::IarScheme::get_schemes(),
            Self::WillplusArc => scheme::willplus_arc::ArcScheme::get_schemes(),
            Self::QliePack => scheme::qliepack::PackScheme::get_schemes(),
            Self::Nekopack => scheme::nekopack::PackScheme::get_schemes(),
            Self::AmusePac => scheme::amusepac::PacScheme::get_schemes(),
            Self::TacticsArc => scheme::tactics_arc::ArcScheme::get_schemes(),
            Self::Link6 => scheme::link6::Link6Scheme::get_schemes(),
            Self::NotRecognized => vec![],
        }
    }
    /// Get all available schemes
    pub fn get_all_schemes() -> Vec<Box<dyn Scheme>> {
        Archive::into_enum_iter()
            .map(|arc| arc.get_schemes())
            .flatten()
            .collect()
    }
}
