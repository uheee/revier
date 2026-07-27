use std::time::Duration;

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use imara_diff::{Algorithm, Diff, InternedInput};
use revier_analysis::overlay::line_diff::diff_lines;

struct DiffFixture {
    name: &'static str,
    old_text: String,
    new_text: String,
}

impl DiffFixture {
    fn bytes(&self) -> usize {
        self.old_text.len() + self.new_text.len()
    }

    fn lines(&self) -> usize {
        count_current_lines(&self.old_text) + count_current_lines(&self.new_text)
    }

    fn change_ratio(&self) -> usize {
        let old_lines = current_lines(&self.old_text);
        let new_lines = current_lines(&self.new_text);
        let changed = old_lines
            .iter()
            .zip(new_lines.iter())
            .filter(|(old, new)| old != new)
            .count()
            + old_lines.len().abs_diff(new_lines.len());
        changed * 100 / old_lines.len().max(new_lines.len()).max(1)
    }
}

fn bench_line_diff(c: &mut Criterion) {
    let fixtures = fixtures();
    let mut group = c.benchmark_group("line_diff");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));

    for fixture in &fixtures {
        let id = format!(
            "{}-{}bytes-{}lines-{}pct",
            fixture.name,
            fixture.bytes(),
            fixture.lines(),
            fixture.change_ratio()
        );
        group.bench_with_input(
            BenchmarkId::new("current_lcs", &id),
            fixture,
            |bench, fixture| {
                bench.iter(|| {
                    let parts = diff_lines(
                        black_box(fixture.old_text.as_str()),
                        black_box(fixture.new_text.as_str()),
                    );
                    black_box(parts.len())
                });
            },
        );
        for algorithm in [Algorithm::Histogram, Algorithm::Myers] {
            group.bench_with_input(
                BenchmarkId::new(format!("imara_{algorithm:?}"), &id),
                fixture,
                |bench, fixture| {
                    bench.iter(|| {
                        let input = InternedInput::new(
                            black_box(fixture.old_text.as_str()),
                            black_box(fixture.new_text.as_str()),
                        );
                        let mut diff = Diff::compute(algorithm, &input);
                        diff.postprocess_lines(&input);
                        black_box((
                            diff.count_removals(),
                            diff.count_additions(),
                            diff.hunks().count(),
                        ))
                    });
                },
            );
        }
    }

    group.finish();
}

fn fixtures() -> Vec<DiffFixture> {
    vec![
        DiffFixture {
            name: "small_code",
            old_text: [
                "fn main() {",
                "    let count = 1;",
                "    println!(\"{count}\");",
                "}",
            ]
            .join("\n"),
            new_text: [
                "fn main() {",
                "    let count = 2;",
                "    let label = \"count\";",
                "    println!(\"{label}: {count}\");",
                "}",
            ]
            .join("\n"),
        },
        DiffFixture {
            name: "medium_repeated",
            old_text: repeated_lines("old", 800, 13),
            new_text: repeated_lines("new", 800, 13),
        },
        DiffFixture {
            name: "large_local_change",
            old_text: local_change_lines(2_000, 980, "old"),
            new_text: local_change_lines(2_000, 980, "new"),
        },
        DiffFixture {
            name: "large_disjoint",
            old_text: numbered_lines("left", 1_200),
            new_text: numbered_lines("right", 1_200),
        },
    ]
}

fn repeated_lines(prefix: &str, count: usize, interval: usize) -> String {
    (0..count)
        .map(|index| {
            if index % interval == 0 {
                "shared-anchor\n".to_string()
            } else {
                format!("{prefix}-{}\n", index % 17)
            }
        })
        .collect()
}

fn local_change_lines(count: usize, change_start: usize, marker: &str) -> String {
    (0..count)
        .map(|index| {
            if (change_start..change_start + 40).contains(&index) {
                format!("{marker}-changed-{index}\n")
            } else {
                format!("shared-{index}\n")
            }
        })
        .collect()
}

fn numbered_lines(prefix: &str, count: usize) -> String {
    (0..count)
        .map(|index| format!("{prefix}-{index}\n"))
        .collect()
}

fn count_current_lines(value: &str) -> usize {
    current_lines(value).len()
}

fn current_lines(value: &str) -> Vec<&str> {
    if value.is_empty() {
        return Vec::new();
    }
    value
        .strip_suffix('\n')
        .unwrap_or(value)
        .split('\n')
        .collect()
}

criterion_group!(benches, bench_line_diff);
criterion_main!(benches);
