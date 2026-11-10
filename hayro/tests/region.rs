//! A region of a page, rendered with `RenderSettings::origin`, is that part
//! of the whole page's render.

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{RenderCache, RenderSettings, render};
use std::sync::Arc;

fn pdf() -> Pdf {
    let content = "0 0 1 rg 20 30 60 40 re f 1 0 0 rg 50 10 30 70 re f";
    let bytes = format!(
        "%PDF-1.4
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >> endobj
4 0 obj << /Length {} >> stream
{content}
endstream endobj
trailer << /Root 1 0 R >>
%%EOF
",
        content.len()
    );
    Pdf::new(Arc::new(bytes.into_bytes())).expect("the PDF loads")
}

#[test]
fn renders_a_region_as_the_whole_page_shows_it() {
    let pdf = pdf();
    let page = &pdf.pages()[0];
    let settings = |width, height, origin| RenderSettings {
        x_scale: 3.0,
        y_scale: 3.0,
        width,
        height,
        origin,
        bg_color: WHITE,
    };
    let cache = RenderCache::new();
    let interp = InterpreterSettings::default();
    let whole = render(page, &cache, &interp, &settings(None, None, (0.0, 0.0)));
    let (x0, y0, w, h) = (70u16, 45u16, 120u16, 90u16);
    let region = render(
        page,
        &cache,
        &interp,
        &settings(Some(w), Some(h), (f32::from(x0), f32::from(y0))),
    );
    assert_eq!((region.width(), region.height()), (w, h));
    let (whole_px, region_px) = (whole.data(), region.data());
    let ww = usize::from(whole.width());
    for y in 0..usize::from(h) {
        for x in 0..usize::from(w) {
            let a = whole_px[(usize::from(y0) + y) * ww + usize::from(x0) + x];
            let b = region_px[y * usize::from(w) + x];
            assert_eq!(a, b, "pixel ({x}, {y})");
        }
    }
}
