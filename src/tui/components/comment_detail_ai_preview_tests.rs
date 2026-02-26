//! Tests for AI preview rendering in the comment detail component.

use super::*;
use crate::ai::{CommentRewriteMode, SideBySideLine};
use crate::tui::components::test_utils::ReviewCommentBuilder;

fn make_preview_ctx<'a>(
    comment: &'a ReviewComment,
    mode: CommentRewriteMode,
    lines: &'a [SideBySideLine],
    has_changes: bool,
) -> CommentDetailViewContext<'a> {
    CommentDetailViewContext {
        selected_comment: Some(comment),
        max_width: 80,
        max_height: 0,
        reply_draft: None,
        reply_draft_ai_preview: Some(ReplyDraftAiPreviewRenderContext {
            mode,
            origin_label: "AI-originated",
            lines,
            has_changes,
        }),
    }
}

#[test]
fn view_renders_ai_preview_section() {
    let component = CommentDetailComponent::new();
    let comment = ReviewCommentBuilder::new(1)
        .author("alice")
        .file_path("src/main.rs")
        .line_number(42)
        .body("nit")
        .build();
    let preview_lines = vec![SideBySideLine {
        original: "old".to_owned(),
        candidate: "new".to_owned(),
    }];
    let ctx = make_preview_ctx(
        &comment,
        CommentRewriteMode::Expand,
        preview_lines.as_slice(),
        true,
    );

    let output = component.view(&ctx);

    assert!(output.contains("AI rewrite preview (expand):"));
    assert!(output.contains("Origin: AI-originated"));
    assert!(output.contains("old || new"));
    assert!(output.contains("Apply: Y  Discard: N"));
}

#[test]
fn view_renders_ai_preview_changed_no_when_candidate_matches_original() {
    let component = CommentDetailComponent::new();
    let comment = ReviewCommentBuilder::new(1)
        .author("alice")
        .file_path("src/main.rs")
        .line_number(42)
        .body("nit")
        .build();
    let preview_lines = vec![SideBySideLine {
        original: "unchanged".to_owned(),
        candidate: "unchanged".to_owned(),
    }];
    let ctx = make_preview_ctx(
        &comment,
        CommentRewriteMode::Reword,
        preview_lines.as_slice(),
        false,
    );

    let output = component.view(&ctx);

    assert!(output.contains("AI rewrite preview (reword):"));
    assert!(output.contains("Origin: AI-originated"));
    assert!(output.contains("Changed: no"));
    assert!(output.contains("unchanged || unchanged"));
    assert!(output.contains("Apply: Y  Discard: N"));
}
