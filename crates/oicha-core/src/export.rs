//! DOCX (OOXML) and plain-text exports, generated entirely by Rust.
use crate::{
    document::{Manuscript, RichText},
    storage::atomic_write,
};
use anyhow::Result;
use std::{
    io::{Cursor, Write},
    path::Path,
};
use zip::{ZipWriter, write::SimpleFileOptions};

fn xml(text: &str) -> String {
    text.chars()
        .filter(|c| matches!(*c as u32, 0x9 | 0xa | 0xd | 0x20..=0xd7ff | 0xe000..=0xfffd | 0x10000..=0x10ffff))
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn paragraph(body: &RichText, start: usize, end: usize) -> String {
    let mut out = String::from(
        "<w:p><w:pPr><w:spacing w:after=\"160\" w:line=\"360\" w:lineRule=\"auto\"/></w:pPr>",
    );
    for run in &body.runs {
        let a = run.range.start.max(start);
        let b = run.range.end.min(end);
        if a >= b {
            continue;
        }
        out.push_str("<w:r><w:rPr>");
        if run.style.bold {
            out.push_str("<w:b/>");
        }
        if run.style.italic {
            out.push_str("<w:i/>");
        }
        if run.style.underline {
            out.push_str("<w:u w:val=\"single\"/>");
        }
        out.push_str("</w:rPr><w:t xml:space=\"preserve\">");
        // Tabs must be represented as OOXML tab nodes, not literal XML whitespace.
        out.push_str(
            &xml(&body.text[a..b]).replace('\t', "</w:t><w:tab/><w:t xml:space=\"preserve\">"),
        );
        out.push_str("</w:t></w:r>");
    }
    out.push_str("</w:p>");
    out
}

pub fn docx_bytes(book: &Manuscript) -> Result<Vec<u8>> {
    book.validate()?;
    let mut document = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>",
    );
    document.push_str(&format!("<w:p><w:pPr><w:pStyle w:val=\"Title\"/></w:pPr><w:r><w:t>{}</w:t></w:r></w:p><w:p><w:r><w:t>{}</w:t></w:r></w:p>", xml(&book.title), xml(&book.author)));
    for chapter in &book.chapters {
        document.push_str(&format!("<w:p><w:pPr><w:pStyle w:val=\"Heading1\"/><w:pageBreakBefore/></w:pPr><w:r><w:t>{}</w:t></w:r></w:p>", xml(&chapter.title)));
        let mut offset = 0;
        for line in chapter.body.text.split('\n') {
            document.push_str(&paragraph(&chapter.body, offset, offset + line.len()));
            offset += line.len() + 1;
        }
    }
    document.push_str("<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\"/></w:sectPr></w:body></w:document>");
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let contents = [
        ("[Content_Types].xml", "<?xml version=\"1.0\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/><Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/></Types>".into()),
        ("_rels/.rels", "<?xml version=\"1.0\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/></Relationships>".into()),
        ("word/_rels/document.xml.rels", "<?xml version=\"1.0\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/></Relationships>".into()),
        ("word/styles.xml", "<?xml version=\"1.0\"?><w:styles xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii=\"Times New Roman\" w:hAnsi=\"Times New Roman\"/><w:sz w:val=\"24\"/><w:lang w:val=\"vi-VN\"/></w:rPr></w:rPrDefault></w:docDefaults><w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/></w:style><w:style w:type=\"paragraph\" w:styleId=\"Title\"><w:name w:val=\"Title\"/><w:basedOn w:val=\"Normal\"/><w:rPr><w:sz w:val=\"48\"/><w:b/></w:rPr></w:style><w:style w:type=\"paragraph\" w:styleId=\"Heading1\"><w:name w:val=\"heading 1\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:outlineLvl w:val=\"0\"/></w:pPr><w:rPr><w:sz w:val=\"32\"/><w:b/></w:rPr></w:style></w:styles>".into()),
        ("word/document.xml", document),
    ];
    for (name, content) in contents {
        writer.start_file(
            name,
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        )?;
        writer.write_all(content.as_bytes())?;
    }
    Ok(writer.finish()?.into_inner())
}

pub fn export_docx(book: &Manuscript, path: &Path) -> Result<()> {
    atomic_write(path, &docx_bytes(book)?)
}
pub fn export_text(book: &Manuscript, path: &Path) -> Result<()> {
    let mut text = format!("{}\n{}\n\n", book.title, book.author);
    for chapter in &book.chapters {
        text.push_str(&format!("{}\n\n{}\n\n", chapter.title, chapter.body.text));
    }
    atomic_write(path, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    #[test]
    fn word_package_preserves_unicode_and_formatting() {
        let mut book = Manuscript::demo();
        book.title = "Gió & <mưa>".into();
        let bytes = docx_bytes(&book).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(archive.len(), 5);
        let mut xml = String::new();
        archive
            .by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        assert!(xml.contains("Gió &amp; &lt;mưa&gt;"));
        assert!(xml.contains("<w:i/>"));
        assert!(xml.contains("Người trở về"));
        assert!(xml.contains("<w:pageBreakBefore/>"));
    }
}
