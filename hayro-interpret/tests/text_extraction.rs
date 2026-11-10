//! What a device gets for text extraction: each glyph run's text state and
//! `TJ` adjustments, each glyph's character code and its font's other
//! codes, and the property lists of marked content (`/ActualText`).

use hayro_interpret::font::{Glyph, GlyphRun, TextRunState};
use hayro_interpret::hayro_syntax::Pdf;
use hayro_interpret::hayro_syntax::object::{Dict, String as PdfString};
use hayro_interpret::{
    BlendMode, ClipPath, Context, Device, DrawMode, DrawProps, Image, ImageDrawProps,
    InterpreterCache, InterpreterSettings, SoftMask, interpret_page,
};
use kurbo::{Affine, BezPath, Rect};

/// A run's state, and its glyphs' codes and adjustments.
type Run = (TextRunState, Vec<(u32, f32)>);
/// A marked content sequence's tag, MCID and `/ActualText`.
type Mark = (Vec<u8>, Option<i32>, Option<Vec<u8>>);

#[derive(Default)]
struct Recorder {
    runs: Vec<Run>,
    marks: Vec<Mark>,
    space_codes: Vec<Option<u32>>,
    space_widths: Vec<Option<f32>>,
}

impl<'a> Device<'a> for Recorder {
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'a>, _: &DrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_glyph_run(&mut self, run: &GlyphRun<'_, 'a>, _: DrawProps<'a>, _: &DrawMode) {
        let glyphs = run
            .glyphs()
            .iter()
            .map(|g| (g.char_code(), g.adjustment()))
            .collect();
        self.runs.push((*run.text_state(), glyphs));
        let first: &Glyph<'_> = &run.glyphs()[0];
        let space = first.char_code_for_unicode(' ');
        self.space_codes.push(space);
        self.space_widths
            .push(space.and_then(|c| first.code_advance_width(c)));
    }
    fn draw_image(&mut self, _: Image<'a, '_>, _: ImageDrawProps<'a>) {}
    fn pop_clip(&mut self) {}
    fn pop_transparency_group(&mut self) {}
    fn begin_marked_content_with_properties(
        &mut self,
        tag: &[u8],
        mcid: Option<i32>,
        properties: Option<&Dict<'_>>,
    ) {
        let actual = properties
            .and_then(|p| p.get::<PdfString<'_>>(b"ActualText".as_slice()))
            .map(|s| s.as_bytes().to_vec());
        self.marks.push((tag.to_vec(), mcid, actual));
    }
}

/// A one-page PDF with Helvetica and the given content stream (hayro
/// rebuilds the cross-reference table).
fn page_with(content: &str, properties: &str) -> Vec<u8> {
    format!(
        "%PDF-1.4
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100]
  /Resources << /Font << /F1 4 0 R >> /Properties << {properties} >> >> /Contents 5 0 R >> endobj
4 0 obj << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> endobj
5 0 obj << /Length {} >> stream
{content}
endstream endobj
trailer << /Root 1 0 R >>
%%EOF
",
        content.len()
    )
    .into_bytes()
}

/// A one-page PDF showing "C" in Helvetica with a `ToUnicode` map that
/// says code 0x43 is "c" (as InDesign writes for All Caps text, with an
/// `/ActualText` of "C" to correct it).
fn page_with_wrong_to_unicode() -> Vec<u8> {
    let cmap = "/CIDInit /ProcSet findresource begin 12 dict begin begincmap
1 begincodespacerange <00> <FF> endcodespacerange
1 beginbfchar <43> <0063> endbfchar
endcmap CMapName currentdict /CMap defineresource pop end end";
    let content = "/Span << /ActualText (C) >> BDC BT /F1 12 Tf (C) Tj ET EMC";
    format!(
        "%PDF-1.4
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100]
  /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >> endobj
4 0 obj << /Type /Font /Subtype /Type1 /BaseFont /Helvetica
  /Encoding /WinAnsiEncoding /ToUnicode 6 0 R >> endobj
5 0 obj << /Length {} >> stream
{content}
endstream endobj
6 0 obj << /Length {} >> stream
{cmap}
endstream endobj
trailer << /Root 1 0 R >>
%%EOF
",
        content.len(),
        cmap.len()
    )
    .into_bytes()
}

fn record(pdf: Vec<u8>) -> Recorder {
    let pdf = Pdf::new(pdf).expect("the PDF loads");
    let page = &pdf.pages()[0];
    let cache = InterpreterCache::new();
    let mut context = Context::new(
        Affine::IDENTITY,
        Rect::new(0.0, 0.0, 200.0, 100.0),
        &cache,
        pdf.xref(),
        InterpreterSettings::default(),
    );
    let mut device = Recorder::default();
    interpret_page(page, &mut context, &mut device);
    device
}

#[test]
fn reports_each_run_s_text_state_and_adjustments() {
    let device = record(page_with(
        "BT /F1 1 Tf 0.5 Tc 2 Tw 80 Tz 3 Ts 10 0 0 10 20 30 Tm [(A) -250 (B) 100 50 (C)] TJ ET",
        "",
    ));
    let [(state, glyphs)] = device.runs.as_slice() else {
        panic!("one run: {:?}", device.runs.len());
    };
    assert_eq!(state.font_size, 1.0);
    assert_eq!(state.char_spacing, 0.5);
    assert_eq!(state.word_spacing, 2.0);
    assert_eq!(state.horizontal_scaling, 80.0);
    assert_eq!(state.rise, 3.0);
    assert_eq!(
        state.text_matrix,
        Affine::new([10.0, 0.0, 0.0, 10.0, 20.0, 30.0])
    );
    assert_eq!(glyphs, &[(65, 0.0), (66, -250.0), (67, 150.0)]);
    // Helvetica's space: code 32, 278 thousandths of an em.
    assert_eq!(device.space_codes, [Some(32)]);
    assert_eq!(device.space_widths, [Some(278.0)]);
}

#[test]
fn tells_a_continuing_run_by_its_line_matrix() {
    let device = record(page_with(
        "BT /F1 10 Tf 20 30 Td (A) Tj (B) Tj 5 0 Td (C) Tj ET",
        "",
    ));
    let continues: Vec<bool> = device
        .runs
        .iter()
        .map(|(s, _)| s.text_matrix != s.line_matrix)
        .collect();
    assert_eq!(continues, [false, true, false]);
}

#[test]
fn passes_marked_content_s_properties() {
    let device = record(page_with(
        "/Span << /ActualText (fi) /MCID 3 >> BDC BT /F1 12 Tf (x) Tj ET EMC \
         /P /MC0 BDC EMC /Artifact BMC EMC",
        "/MC0 << /ActualText (named) >>",
    ));
    assert_eq!(
        device.marks,
        [
            (b"Span".to_vec(), Some(3), Some(b"fi".to_vec())),
            (b"P".to_vec(), None, Some(b"named".to_vec())),
            (b"Artifact".to_vec(), None, None),
        ]
    );
}

#[test]
fn finds_a_code_by_the_encoding_not_a_wrong_to_unicode_map() {
    struct Lookup(Vec<[Option<u32>; 2]>);
    impl<'a> Device<'a> for Lookup {
        fn draw_path(&mut self, _: &BezPath, _: DrawProps<'a>, _: &DrawMode) {}
        fn push_clip_path(&mut self, _: &ClipPath) {}
        fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
        fn draw_glyph_run(&mut self, run: &GlyphRun<'_, 'a>, _: DrawProps<'a>, _: &DrawMode) {
            let glyph: &Glyph<'_> = &run.glyphs()[0];
            self.0.push([
                glyph.char_code_for_unicode('C'),
                glyph.char_code_for_unicode('c'),
            ]);
        }
        fn draw_image(&mut self, _: Image<'a, '_>, _: ImageDrawProps<'a>) {}
        fn pop_clip(&mut self) {}
        fn pop_transparency_group(&mut self) {}
    }
    let pdf = Pdf::new(page_with_wrong_to_unicode()).expect("the PDF loads");
    let cache = InterpreterCache::new();
    let mut context = Context::new(
        Affine::IDENTITY,
        Rect::new(0.0, 0.0, 200.0, 100.0),
        &cache,
        pdf.xref(),
        InterpreterSettings::default(),
    );
    let mut device = Lookup(Vec::new());
    interpret_page(&pdf.pages()[0], &mut context, &mut device);
    // "C" isn't in the map, so it's found by WinAnsi's glyph names, at
    // 0x43 (not by the map's wrong "c" for 0x43); "c" is the map's.
    assert_eq!(device.0, [[Some(0x43), Some(0x43)]]);
}
