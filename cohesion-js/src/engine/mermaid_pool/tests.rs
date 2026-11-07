use std::time::Duration;

use cohesion_core::protocol::{MermaidRefusal, RenderedMermaid};

use super::{MermaidPool, QUEUE_LIMIT};
use crate::sandbox::SandboxOptions;

const SMALL: &str = "flowchart LR\n  Seeds --> Seedlings --> Beds --> Harvest\n";

fn pool(options: SandboxOptions) -> MermaidPool {
    MermaidPool::with_limits(1, options, QUEUE_LIMIT)
}

fn draw(pool: &MermaidPool, source: &str, width: Option<u32>) -> RenderedMermaid {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(pool.render(source.to_string(), width))
}

fn svg(rendered: RenderedMermaid) -> (String, f64) {
    match rendered {
        RenderedMermaid::Svg { svg, width, .. } => (svg, width),
        RenderedMermaid::Refused { reason, message } => {
            panic!("refused: {reason:?}: {message}")
        }
    }
}

fn reason(rendered: RenderedMermaid) -> MermaidRefusal {
    match rendered {
        RenderedMermaid::Refused { reason, .. } => reason,
        RenderedMermaid::Svg { .. } => panic!("it drew"),
    }
}

/// As many edges as Pele's limits let through, between few nodes: the slowest
/// and largest layout found.
fn densest() -> String {
    let mut text = String::from("flowchart LR\n");
    let mut seed = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = |nodes: u64| {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) % nodes
    };
    for _ in 0..4900 {
        text += &format!("n{}-->n{}\n", next(100), next(100));
    }
    text
}

#[test]
fn every_kind_draws_with_no_dom_and_no_palette() {
    let pool = pool(SandboxOptions::MERMAID);
    for source in [
        SMALL,
        "sequenceDiagram\n  A->>B: hello\n  B-->>A: hi\n",
        "stateDiagram-v2\n  [*] --> Idle\n  Idle --> [*]\n",
        "classDiagram\n  class A\n  A --> B\n",
        "erDiagram\n  A ||--o{ B : has\n",
        "xychart-beta\n  x-axis [a, b]\n  bar [1, 2]\n",
        "pie\n  \"a\" : 1\n  \"b\" : 2\n",
        "gantt\n  dateFormat YYYY-MM-DD\n  Start :2026-11-04, 1d\n",
        "flowchart LR\n  A[\"开始 🚀\"] --> B[\"Привет\"]\n",
    ] {
        let (svg, _) = svg(draw(&pool, source, Some(720)));
        let (root, body) = svg.split_at(svg.find('>').unwrap());
        // It scales rather than asking again, and a theme is all CSS variables.
        assert!(root.contains("viewBox="), "{root}");
        assert!(root.contains("max-width:100%;height:auto"), "{root}");
        assert!(root.contains("var(--pele-fg,"), "{root}");
        for literal in ["fill=\"#", "stroke=\"#", "fill=\"rgb", "stroke=\"rgb"] {
            assert!(!body.contains(literal), "{literal} in {source}: {body}");
        }
    }
}

#[test]
fn the_width_is_what_the_diagram_is_laid_out_for() {
    let pool = pool(SandboxOptions::MERMAID);
    let wide = "flowchart LR\n  A[One long label] --> B[Another long label] --> C[A third long label] --> D[The last long label]\n";
    let (_, natural) = svg(draw(&pool, wide, None));
    let (_, narrow) = svg(draw(&pool, wide, Some(360)));
    assert!(narrow < natural, "{narrow} against {natural}");
}

#[test]
fn a_diagram_whose_look_needs_ids_gets_its_own() {
    let pool = pool(SandboxOptions::MERMAID);
    let id = |source: &str| {
        let (svg, _) = svg(draw(&pool, source, None));
        let start = svg.find(" id=\"").expect("a Sankey's gradient has an id") + 5;
        svg[start..start + svg[start..].find('"').unwrap()].to_string()
    };
    assert_ne!(id("sankey-beta\nA,B,1\n"), id("sankey-beta\nA,B,2\n"));
}

#[test]
fn what_will_not_draw_is_refused_and_the_worker_carries_on() {
    let pool = pool(SandboxOptions::MERMAID);
    for (source, expected) in [
        ("", MermaidRefusal::Unsupported),
        ("this is not a diagram", MermaidRefusal::Unsupported),
        ("flowchart TD\n  A[unclosed --> B\n", MermaidRefusal::Syntax),
        ("flowchart LR\n  A\0B --> C\n", MermaidRefusal::Syntax),
    ] {
        assert_eq!(reason(draw(&pool, source, None)), expected, "{source:?}");
        svg(draw(&pool, SMALL, None));
    }
    let oversized = format!("flowchart LR\n{}", "A --> B\n".repeat(20_000));
    assert_eq!(
        reason(draw(&pool, &oversized, Some(360))),
        MermaidRefusal::TooLarge
    );
    svg(draw(&pool, SMALL, None));
}

#[test]
fn a_source_larger_than_the_runtime_is_refused_and_the_worker_carries_on() {
    let pool = pool(SandboxOptions::MERMAID);
    let enormous = "x".repeat(128 * 1024 * 1024);
    assert_eq!(
        reason(draw(&pool, &enormous, None)),
        MermaidRefusal::TooLarge
    );
    svg(draw(&pool, SMALL, None));
}

#[test]
fn deep_nesting_ends_in_a_reply_and_the_worker_carries_on() {
    let pool = pool(SandboxOptions::MERMAID);
    let depth = 3000;
    let nested = format!(
        "flowchart TD\n{}a-->b\n{}",
        (0..depth)
            .map(|i| format!("subgraph s{i}\n"))
            .collect::<String>(),
        "end\n".repeat(depth)
    );
    // Either reply will do; a crash would end the test binary.
    _ = draw(&pool, &nested, None);
    svg(draw(&pool, SMALL, None));
}

#[test]
fn a_slow_layout_is_stopped_and_the_worker_starts_afresh() {
    let pool = pool(SandboxOptions {
        memory_limit_bytes: SandboxOptions::MERMAID.memory_limit_bytes,
        time_limit: Duration::from_millis(500),
    });
    assert_eq!(
        reason(draw(&pool, &densest(), Some(360))),
        MermaidRefusal::TooSlow
    );
    svg(draw(&pool, SMALL, None));
}

#[test]
fn running_out_of_memory_is_refused_and_the_worker_starts_afresh() {
    // Room to load Pele, not to lay this out, with Pele's source in the heap.
    let pool = pool(SandboxOptions {
        memory_limit_bytes: 12 * 1024 * 1024,
        time_limit: SandboxOptions::MERMAID.time_limit,
    });
    assert_eq!(
        reason(draw(&pool, &densest(), None)),
        MermaidRefusal::TooLarge
    );
    svg(draw(&pool, SMALL, None));
}

#[test]
fn the_densest_diagram_found_fits_the_limits() {
    let pool = pool(SandboxOptions::MERMAID);
    svg(draw(&pool, &densest(), None));
}

#[test]
fn a_diagram_that_waited_too_long_is_refused_as_busy() {
    let pool = MermaidPool::with_limits(1, SandboxOptions::MERMAID, Duration::ZERO);
    assert_eq!(reason(draw(&pool, SMALL, None)), MermaidRefusal::Busy);
}

#[test]
fn the_vendored_pele_is_the_documented_version() {
    let vendored = include_str!("../../../vendor/pele/pele.global.js");
    let readme = include_str!("../../../vendor/pele/README.md");
    let version = readme
        .split("- Version: ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .expect("the README names Pele's version");
    assert!(
        vendored.starts_with(&format!("/*! Pele {version} ")),
        "vendor/pele/README.md says Pele {version}; update it with the file"
    );
}
