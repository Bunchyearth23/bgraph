use bgraph::{analyze_source, render_svg};

#[test]
fn orthogonal_ports_and_waypoints_survive_more_than_sixty_obstacles() {
    let mut source = String::from(
        "node a { size: 100 100 }\nnode b { size: 100 100 }\nnode wall { size: 70 200 }\nlayout {\n place a at (0, 0)\n place b at (400, 0)\n place wall at (190, -80)\n}\na -> b via (200, -120) { router: ortho from: right 2 of 3 to: left 4 of 4 }\n",
    );
    for i in 0..70 {
        source.push_str(&format!(
            "node far{i} {{ size: 20 20 }}\nlayout {{ place far{i} at ({}, 1000) }}\n",
            i * 30
        ));
    }
    let analysis = analyze_source(&source);
    assert!(!analysis.has_errors(), "{:?}", analysis.diagnostics);
    assert!(
        !analysis
            .diagnostics
            .iter()
            .any(|diag| diag.message.contains("passes through a box")),
        "{:?}",
        analysis.diagnostics
    );
    let layout = analysis.layout.unwrap();
    let routes = render_svg::route_all(&layout);
    let points = &routes[0].points;
    assert_eq!(points[0], (100.0, 50.0));
    assert_eq!(*points.last().unwrap(), (400.0, 80.0));
    assert!(
        points
            .windows(2)
            .all(|p| p[0].0 == p[1].0 || p[0].1 == p[1].1)
    );
    assert!(points.windows(2).any(|p| {
        (p[0].0.min(p[1].0)..=p[0].0.max(p[1].0)).contains(&200.0)
            && (p[0].1.min(p[1].1)..=p[0].1.max(p[1].1)).contains(&-120.0)
    }));
}
