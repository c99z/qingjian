//! 核对向上展开的行内容、高亮与拼音 / 页码位置，保证视觉倒序不改变候选身份。

use crate::fonts::FontLibrary;
use crate::frame::{Frame, Preedit, Row, Tone};
use crate::layout::Layout;
use crate::renderer::{Rendered, Renderer};
use crate::theme::Theme;

fn strip(rendered: &Rendered, y: u32, height: u32) -> Vec<u8> {
    let stride = rendered.pixmap.width() as usize * 4;
    rendered.pixmap.data()[y as usize * stride..(y + height) as usize * stride].to_vec()
}

#[test]
fn upward_layout_moves_rows_with_their_indices_annotations_and_highlight() {
    let Ok(library) = FontLibrary::system("zh-CN") else {
        return;
    };
    let mut renderer = Renderer::new(library);
    for theme in [Theme::light(), Theme::dark()] {
        for scale in [1.0, 2.0] {
            for count in [1, 3, 9] {
                let mut frame = Frame {
                    preedit: Some(Preedit::plain("dao'zhe", 7)),
                    rows: (0..count)
                        .map(|i| {
                            let mut row = Row::plain(i, format!("候选 {i}"));
                            row.annotation = vec![(format!("gloss {i}"), Tone::Gloss)];
                            row
                        })
                        .collect(),
                    highlighted: Some(count / 2),
                    footer: Some("2/3".into()),
                    ..Frame::default()
                };
                let normal = renderer
                    .render(&frame, Layout::Vertical, &theme, scale, None)
                    .unwrap();
                frame.vertical_reversed = true;
                let reversed = renderer
                    .render(&frame, Layout::Vertical, &theme, scale, None)
                    .unwrap();
                assert_eq!(normal.content_width, reversed.content_width);
                assert_eq!(normal.content_height, reversed.content_height);
                assert_eq!(
                    renderer.content_size(&frame, Layout::Vertical, &theme, scale),
                    (reversed.content_width, reversed.content_height)
                );
                let padding = (theme.padding * scale) as u32;
                let top_height =
                    ((theme.annotation_font.line_height + theme.row_padding * 2.0) * scale) as u32;
                let footer_height =
                    ((theme.index_font.line_height + theme.row_padding) * scale) as u32;
                let row_height =
                    ((theme.text_font.line_height + theme.row_padding * 2.0) * scale) as u32;
                let rows_height = count as u32 * row_height;
                for i in 0..count as u32 {
                    assert_eq!(
                        strip(&normal, padding + top_height + i * row_height, row_height),
                        strip(
                            &reversed,
                            padding + footer_height + (count as u32 - 1 - i) * row_height,
                            row_height,
                        ),
                        "candidate {i}, count {count}, scale {scale}",
                    );
                }
                assert_eq!(
                    strip(&normal, padding, top_height),
                    strip(&reversed, padding + footer_height + rows_height, top_height),
                );
                assert_eq!(
                    strip(&normal, padding + top_height + rows_height, footer_height),
                    strip(&reversed, padding, footer_height),
                );
            }
        }
    }
}

#[test]
fn upward_layout_without_preedit_or_footer_keeps_first_candidate_at_bottom() {
    let Ok(library) = FontLibrary::system("zh-CN") else {
        return;
    };
    let mut renderer = Renderer::new(library);
    let theme = Theme::light();
    let mut frame = Frame {
        rows: vec![Row::plain(0, "first"), Row::plain(1, "second")],
        highlighted: Some(0),
        ..Frame::default()
    };
    let normal = renderer
        .render(&frame, Layout::Vertical, &theme, 1.0, None)
        .unwrap();
    frame.vertical_reversed = true;
    let reversed = renderer
        .render(&frame, Layout::Vertical, &theme, 1.0, None)
        .unwrap();
    let padding = theme.padding as u32;
    let row_height = (theme.text_font.line_height + theme.row_padding * 2.0) as u32;
    assert_eq!(
        strip(&normal, padding, row_height),
        strip(&reversed, padding + row_height, row_height)
    );
    assert_eq!(
        strip(&normal, padding + row_height, row_height),
        strip(&reversed, padding, row_height)
    );
}

#[test]
fn reversal_flag_does_not_change_horizontal_or_matrix_layout() {
    let Ok(library) = FontLibrary::system("zh-CN") else {
        return;
    };
    let mut renderer = Renderer::new(library);
    for columns in [0, 2] {
        let mut frame = Frame {
            preedit: Some(Preedit::plain("nihao", 5)),
            rows: vec![
                Row::plain(0, "first"),
                Row::plain(1, "second"),
                Row::plain(2, "third"),
            ],
            highlighted: Some(1),
            columns,
            ..Frame::default()
        };
        let normal = renderer
            .render(&frame, Layout::Horizontal, &Theme::light(), 1.0, None)
            .unwrap();
        frame.vertical_reversed = true;
        let flagged = renderer
            .render(&frame, Layout::Horizontal, &Theme::light(), 1.0, None)
            .unwrap();
        assert_eq!(normal.pixmap, flagged.pixmap);
    }
}
