//! Output path computation: mirrored folder structure + optional tag and signature.
use crate::models::{EncodeSettings, NamingSettings};
use std::path::{Path, PathBuf};

const ILLEGAL: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

pub fn sanitize(s: &str) -> String {
    s.chars().filter(|c| !ILLEGAL.contains(c) && !c.is_control()).collect()
}

pub fn render_tag(template: &str, settings: &EncodeSettings, out_h: u32) -> String {
    let res = if out_h > 0 { format!("{out_h}p") } else { "src".into() };
    sanitize(
        &template
            .replace("{res}", &res)
            .replace("{codec}", settings.codec.label())
            .replace("{crf}", &settings.effective_crf().to_string()),
    )
}

pub fn output_file_name(source: &Path, settings: &EncodeSettings, naming: &NamingSettings, out_h: u32) -> String {
    let stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let mut name = stem.to_string();
    if naming.add_tag {
        let tag = render_tag(&naming.tag_template, settings, out_h);
        if !tag.trim().is_empty() {
            name.push(' ');
            name.push_str(tag.trim());
        }
    }
    if naming.add_signature {
        name.push_str(&sanitize(&naming.signature));
    }
    format!("{}.{}", name.trim_end(), settings.container.ext())
}

/// Full output path. `root` is the folder the source was added from; when preserving
/// structure the source's path relative to root is mirrored under `output_dir`.
pub fn output_path(source: &Path, root: &Path, output_dir: &Path, preserve_structure: bool, settings: &EncodeSettings, naming: &NamingSettings, out_h: u32) -> PathBuf {
    let file = output_file_name(source, settings, naming, out_h);
    let rel_dir = if preserve_structure {
        source.parent().and_then(|p| p.strip_prefix(root).ok()).map(|p| p.to_path_buf()).unwrap_or_default()
    } else {
        PathBuf::new()
    };
    output_dir.join(rel_dir).join(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    #[test]
    fn plain_name_by_default() {
        let s = EncodeSettings::default();
        let n = NamingSettings::default();
        assert_eq!(output_file_name(Path::new("/m/The Wire S01E03.mp4"), &s, &n, 480), "The Wire S01E03.mkv");
    }

    #[test]
    fn tag_and_signature() {
        let s = EncodeSettings::default();
        let n = NamingSettings { add_tag: true, tag_template: "[{res} {codec}]".into(), add_signature: true, signature: "_myname".into() };
        assert_eq!(output_file_name(Path::new("/m/The Wire S01E03.mkv"), &s, &n, 480), "The Wire S01E03 [480p HEVC]_myname.mkv");
        let n2 = NamingSettings { add_tag: false, add_signature: true, signature: " [MV]".into(), ..Default::default() };
        assert_eq!(output_file_name(Path::new("/m/a.mkv"), &s, &n2, 480), "a [MV].mkv");
    }

    #[test]
    fn illegal_chars_stripped() {
        let s = EncodeSettings::default();
        let n = NamingSettings { add_signature: true, signature: "a/b:c?".into(), ..Default::default() };
        assert_eq!(output_file_name(Path::new("/m/a.mkv"), &s, &n, 480), "aabc.mkv");
    }

    #[test]
    fn mirrors_structure() {
        let s = EncodeSettings::default();
        let n = NamingSettings::default();
        let p = output_path(Path::new("/lib/The Wire/Season 02/E01.mkv"), Path::new("/lib/The Wire"), Path::new("/out"), true, &s, &n, 480);
        assert_eq!(p, PathBuf::from("/out/Season 02/E01.mkv"));
        let flat = output_path(Path::new("/lib/The Wire/Season 02/E01.mkv"), Path::new("/lib/The Wire"), Path::new("/out"), false, &s, &n, 480);
        assert_eq!(flat, PathBuf::from("/out/E01.mkv"));
    }
}
