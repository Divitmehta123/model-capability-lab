use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Capability {
    ToolCalls,
    Vision,
    JsonSchema,
    Streaming,
    LongContext,
}
impl Capability {
    pub const ALL: [Self; 5] = [
        Self::ToolCalls,
        Self::Vision,
        Self::JsonSchema,
        Self::Streaming,
        Self::LongContext,
    ];
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "tool_calls" => Some(Self::ToolCalls),
            "vision" => Some(Self::Vision),
            "json_schema" => Some(Self::JsonSchema),
            "streaming" => Some(Self::Streaming),
            "long_context" => Some(Self::LongContext),
            _ => None,
        }
    }
    pub const fn key(self) -> &'static str {
        match self {
            Self::ToolCalls => "tool_calls",
            Self::Vision => "vision",
            Self::JsonSchema => "json_schema",
            Self::Streaming => "streaming",
            Self::LongContext => "long_context",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Pass,
    Fail,
    Unsupported,
}
impl Outcome {
    fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "pass" => Some(Self::Pass),
            "fail" => Some(Self::Fail),
            "unsupported" => Some(Self::Unsupported),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observation {
    pub provider: String,
    pub model: String,
    pub scenario: String,
    pub capability: Capability,
    pub weight: u16,
    pub outcome: Outcome,
    pub latency_ms: u32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Evaluation {
    pub provider: String,
    pub model: String,
    pub score: u16,
    pub maximum: u16,
    pub passed: usize,
    pub total: usize,
    pub p95_latency_ms: u32,
    pub gaps: Vec<Capability>,
    pub failed_scenarios: Vec<String>,
}

pub fn parse_tsv(input: &str) -> Result<Vec<Observation>, String> {
    let mut lines = input
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'));
    let header = lines.next().ok_or("input is empty")?;
    if header.trim() != "provider\tmodel\tscenario\tcapability\tweight\toutcome\tlatency_ms" {
        return Err("expected TSV header: provider, model, scenario, capability, weight, outcome, latency_ms".into());
    }
    let mut observations = Vec::new();
    let mut keys = BTreeSet::new();
    for (offset, line) in lines.enumerate() {
        let line_number = offset + 2;
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 7 {
            return Err(format!(
                "line {line_number}: expected 7 tab-separated fields"
            ));
        }
        let capability = Capability::parse(fields[3])
            .ok_or_else(|| format!("line {line_number}: unknown capability `{}`", fields[3]))?;
        let weight = fields[4]
            .parse::<u16>()
            .map_err(|_| format!("line {line_number}: invalid weight"))?;
        if weight == 0 {
            return Err(format!("line {line_number}: weight must be positive"));
        }
        let outcome = Outcome::parse(fields[5])
            .ok_or_else(|| format!("line {line_number}: invalid outcome `{}`", fields[5]))?;
        let latency_ms = fields[6]
            .parse::<u32>()
            .map_err(|_| format!("line {line_number}: invalid latency"))?;
        if fields[..3].iter().any(|value| value.trim().is_empty()) {
            return Err(format!(
                "line {line_number}: identity fields cannot be empty"
            ));
        }
        if !keys.insert((fields[0], fields[1], fields[2])) {
            return Err(format!(
                "line {line_number}: duplicate provider/model/scenario observation"
            ));
        }
        observations.push(Observation {
            provider: fields[0].into(),
            model: fields[1].into(),
            scenario: fields[2].into(),
            capability,
            weight,
            outcome,
            latency_ms,
        });
    }
    if observations.is_empty() {
        Err("input contains no observations".into())
    } else {
        Ok(observations)
    }
}

pub fn evaluate(observations: &[Observation]) -> Vec<Evaluation> {
    let mut groups: BTreeMap<(&str, &str), Vec<&Observation>> = BTreeMap::new();
    for item in observations {
        groups
            .entry((&item.provider, &item.model))
            .or_default()
            .push(item);
    }
    let mut results = groups
        .into_iter()
        .map(|((provider, model), items)| {
            let maximum = items.iter().map(|item| item.weight).sum();
            let score = items
                .iter()
                .filter(|item| item.outcome == Outcome::Pass)
                .map(|item| item.weight)
                .sum();
            let mut latencies = items
                .iter()
                .filter(|item| item.outcome != Outcome::Unsupported)
                .map(|item| item.latency_ms)
                .collect::<Vec<_>>();
            latencies.sort_unstable();
            let p95_index = (latencies.len() * 95).div_ceil(100).saturating_sub(1);
            let p95_latency_ms = latencies.get(p95_index).copied().unwrap_or(0);
            let gaps = Capability::ALL
                .into_iter()
                .filter(|capability| {
                    items
                        .iter()
                        .any(|item| item.capability == *capability && item.outcome != Outcome::Pass)
                })
                .collect();
            let failed_scenarios = items
                .iter()
                .filter(|item| item.outcome != Outcome::Pass)
                .map(|item| item.scenario.clone())
                .collect();
            Evaluation {
                provider: provider.into(),
                model: model.into(),
                score,
                maximum,
                passed: items
                    .iter()
                    .filter(|item| item.outcome == Outcome::Pass)
                    .count(),
                total: items.len(),
                p95_latency_ms,
                gaps,
                failed_scenarios,
            }
        })
        .collect::<Vec<_>>();
    results.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(a.p95_latency_ms.cmp(&b.p95_latency_ms))
            .then(a.provider.cmp(&b.provider))
            .then(a.model.cmp(&b.model))
    });
    results
}

pub fn eligible<'a>(
    results: &'a [Evaluation],
    minimum: u16,
    required: &[Capability],
) -> Vec<&'a Evaluation> {
    results
        .iter()
        .filter(|result| {
            result.score >= minimum
                && required
                    .iter()
                    .all(|capability| !result.gaps.contains(capability))
        })
        .collect()
}
fn escape_json(value: &str) -> String {
    value
        .chars()
        .flat_map(|c| match c {
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '"' => "\\\"".chars().collect(),
            '\n' => "\\n".chars().collect(),
            '\r' => "\\r".chars().collect(),
            '\t' => "\\t".chars().collect(),
            c => vec![c],
        })
        .collect()
}
pub fn render_text(results: &[Evaluation]) -> String {
    results
        .iter()
        .map(|r| {
            format!(
                "{:<12} {:<24} {:>3}/{:<3}  {:>2}/{:<2} pass  p95 {:>5}ms  gaps: {}",
                r.provider,
                r.model,
                r.score,
                r.maximum,
                r.passed,
                r.total,
                r.p95_latency_ms,
                if r.gaps.is_empty() {
                    "none".into()
                } else {
                    r.gaps.iter().map(|c| c.key()).collect::<Vec<_>>().join(",")
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
pub fn render_markdown(results: &[Evaluation]) -> String {
    let mut out = String::from(
        "# Capability scorecard\n\n| Provider | Model | Score | Passed | p95 latency | Explicit profile gaps |\n| --- | --- | ---: | ---: | ---: | --- |\n",
    );
    for r in results {
        out.push_str(&format!(
            "| {} | {} | {} / {} | {} / {} | {} ms | {} |\n",
            r.provider,
            r.model,
            r.score,
            r.maximum,
            r.passed,
            r.total,
            r.p95_latency_ms,
            if r.gaps.is_empty() {
                "none".into()
            } else {
                r.gaps
                    .iter()
                    .map(|c| c.key())
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        ));
    }
    out.push_str("\nScores describe this declared fixture set, not universal model quality. A gap means routing must require an explicit capability or compatibility profile.\n");
    out
}
pub fn render_json(results: &[Evaluation]) -> String {
    let rows=results.iter().map(|r|format!("{{\"provider\":\"{}\",\"model\":\"{}\",\"score\":{},\"maximum\":{},\"passed\":{},\"total\":{},\"p95_latency_ms\":{},\"gaps\":[{}],\"failed_scenarios\":[{}]}}",escape_json(&r.provider),escape_json(&r.model),r.score,r.maximum,r.passed,r.total,r.p95_latency_ms,r.gaps.iter().map(|c|format!("\"{}\"",c.key())).collect::<Vec<_>>().join(","),r.failed_scenarios.iter().map(|s|format!("\"{}\"",escape_json(s))).collect::<Vec<_>>().join(","))).collect::<Vec<_>>().join(",");
    format!("{{\"schema_version\":1,\"evaluations\":[{rows}]}}")
}

#[cfg(test)]
mod tests {
    use super::*;
    const TSV: &str = "provider\tmodel\tscenario\tcapability\tweight\toutcome\tlatency_ms\nA\talpha\ttools\ttool_calls\t30\tpass\t120\nA\talpha\tvision\tvision\t20\tfail\t400\nB\tbeta\ttools\ttool_calls\t30\tunsupported\t0\n";
    #[test]
    fn parses_and_ranks_weighted_observations() {
        let r = evaluate(&parse_tsv(TSV).unwrap());
        assert_eq!(r[0].score, 30);
        assert_eq!(r[0].gaps, vec![Capability::Vision]);
    }
    #[test]
    fn rejects_duplicate_observations() {
        assert!(
            parse_tsv(&(TSV.to_string() + "A\talpha\ttools\ttool_calls\t30\tpass\t1\n"))
                .unwrap_err()
                .contains("duplicate")
        );
    }
    #[test]
    fn eligibility_enforces_score_and_capability() {
        let r = evaluate(&parse_tsv(TSV).unwrap());
        assert!(
            eligible(&r, 30, &[Capability::ToolCalls])
                .iter()
                .any(|e| e.model == "alpha")
        );
        assert!(eligible(&r, 31, &[]).is_empty());
    }
    #[test]
    fn json_is_versioned() {
        let value = render_json(&evaluate(&parse_tsv(TSV).unwrap()));
        assert!(value.starts_with("{\"schema_version\":1"));
        assert!(value.contains("\"p95_latency_ms\":400"));
    }
    #[test]
    fn markdown_states_scope_limit() {
        assert!(
            render_markdown(&evaluate(&parse_tsv(TSV).unwrap()))
                .contains("not universal model quality")
        );
    }
}
