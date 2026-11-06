use super::*;
use history::tests::history;

fn panel(values: &[(u64, u64, Option<f64>)]) -> Panel {
    let mut panel = Panel::new(Some(PathBuf::new()));
    panel.history = Some(history(values));
    panel.next_poll = Instant::now() + Duration::from_secs(60);
    panel.poll();
    panel
}

fn frame(panel: &mut Panel, context: &egui::Context, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 800.0),
        )),
        events,
        ..Default::default()
    };
    context
        .run_ui(input, |ui| panel.show(ui))
        .textures_delta
        .clear();
}

#[test]
fn zoom_disables_follow_and_keeps_bounds_when_series_changes() {
    let mut panel = panel(&[(1, 1, Some(1.0)), (1, 100, Some(2.0))]);
    let context = egui::Context::default();
    frame(&mut panel, &context, Vec::new());
    let center = panel.plot_rect.unwrap().center();
    frame(
        &mut panel,
        &context,
        vec![egui::Event::PointerMoved(center), egui::Event::Zoom(8.0)],
    );
    assert!(!panel.follow);
    let zoomed = panel.range.unwrap();
    assert!(zoomed[1] - zoomed[0] < 50.0);
    panel.segments[0].points.push(history::Point {
        tick: 10_000,
        value: 1000.0,
        window: None,
    });
    frame(&mut panel, &context, Vec::new());
    assert_eq!(panel.range, Some(zoomed));
}

#[test]
fn following_includes_windows_where_the_selected_metric_is_absent() {
    let mut panel = panel(&[(1, 1, Some(1.0)), (1, 500, None)]);
    frame(&mut panel, &egui::Context::default(), Vec::new());
    let bounds = panel.plot_bounds.unwrap();
    assert!(bounds[0] <= 1.0 && bounds[1] >= 500.0);
}

#[test]
fn changing_the_plot_range_does_not_wait_for_file_polling() {
    let mut panel = panel(&[(1, 1, Some(1.0)), (1, 100, Some(2.0))]);
    panel.follow = false;
    panel.range = Some([40.0, 60.0]);
    panel.poll();
    assert_eq!(panel.plotted.as_ref().unwrap().range, panel.range);
}
