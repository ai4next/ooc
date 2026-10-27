//! Japanese and Korean catalogs (`i18n/*.tsv`, keyed by the English text).
//! English and Chinese are inline in the code; see `oo_base::i18n`.

use std::sync::Once;

use oo_base::{Lang, Tr};

pub const JA: &str = include_str!("../../../i18n/ja.tsv");
pub const KO: &str = include_str!("../../../i18n/ko.tsv");

/// Installs the catalogs once. Call at startup and at the top of tests that
/// read translated text.
pub fn install() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        oo_base::i18n::install(Lang::Ja, JA).expect("i18n/ja.tsv is malformed");
        oo_base::i18n::install(Lang::Ko, KO).expect("i18n/ko.tsv is malformed");
    });
}

/// Shared `oo_base` text shown by ooc. The catalog tool only scans this
/// game's crates, so the strings are mirrored here; a test keeps them equal.
pub const SHARED: &[Tr] = &[
    Tr::new("Grandpa Juye", "橘爷"),
    Tr::new("Huajuan", "花卷"),
    Tr::new("Douya", "豆芽"),
    Tr::new("Granny Yin", "银婆"),
    Tr::new("Ali", "阿砾"),
    Tr::new("Dr. Bohe", "薄荷"),
    Tr::new("Cen Li", "岑理"),
    Tr::new("Station master", "驿站长"),
    Tr::new("Schoolteacher", "小学老师"),
    Tr::new("Postal courier", "邮差"),
    Tr::new("Well keeper", "守井人"),
    Tr::new("Caravan guide", "驼队向导"),
    Tr::new("Clinic doctor", "诊所医生"),
    Tr::new("Academy examiner", "科学院审查官"),
];

#[cfg(test)]
mod tests {
    use oo_base::Cat;

    #[test]
    fn shared_text_mirrors_oo_base() {
        for i in Cat::ALL {
            for tr in [i.name(), i.role()] {
                assert!(super::SHARED.contains(&tr), "mirror {tr:?} in SHARED");
            }
        }
    }

    #[test]
    fn catalogs_parse() {
        assert!(oo_base::i18n::parse(super::JA).is_ok());
        assert!(oo_base::i18n::parse(super::KO).is_ok());
        super::install();
    }
}
