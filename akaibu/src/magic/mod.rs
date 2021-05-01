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
            _ => Self::NotRecognized,
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
