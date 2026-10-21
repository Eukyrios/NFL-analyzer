//! Human-readable labels shared by the filters, list rows and charts.

pub fn pass_result(code: &str) -> &'static str {
    match code {
        "C" => "Complete",
        "I" => "Incomplete",
        "S" => "Sack",
        "IN" => "Interception",
        "R" => "Scramble",
        _ => "Unknown",
    }
}

pub fn down(d: i64) -> String {
    match d {
        1 => "1st".into(),
        2 => "2nd".into(),
        3 => "3rd".into(),
        n => format!("{n}th"),
    }
}

pub fn quarter(q: i64) -> String {
    if q <= 4 { format!("Q{q}") } else { "OT".into() }
}

/// `DESIGNED_ROLLOUT_RIGHT` → `Designed rollout right`.
pub fn title_case(s: &str) -> String {
    let lower = s.replace('_', " ").to_lowercase();
    let mut c = lower.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
}

/// One-line descriptions from `data-README.md`.
pub fn coverage_description(c: &str) -> &'static str {
    match c {
        "Cover-0" => "Man across the board, no deep defenders",
        "Cover-1" => "Man coverage with a single-high safety",
        "Cover-2" => "Two deep safeties, zone principles",
        "2-Man" => "Two deep safeties, man principles",
        "Cover-3" => "Any 3 Deep, 4 Under concept",
        "Quarters" => "4 Deep, 3 Under on both halves",
        "Cover-6" => "Quarters on one half, Cover-2 on the other",
        "Bracket" => "Two defenders bracket a receiver",
        "Goal Line" => "Goal line defense",
        "Red Zone" => "Red-zone specific call",
        "Prevent" => "End of half/game prevent defense",
        "Miscellaneous" => "Doesn't fit another category",
        _ => "",
    }
}

pub fn coverage_kind(c: &str) -> &'static str {
    match c {
        "Cover-0" | "Cover-1" | "2-Man" => "Man",
        "Cover-2" | "Cover-3" | "Quarters" | "Cover-6" => "Zone",
        _ => "Other",
    }
}

pub fn position(p: &str) -> &'static str {
    match p {
        "QB" => "Quarterback",
        "RB" => "Running back",
        "FB" => "Fullback",
        "WR" => "Wide receiver",
        "TE" => "Tight end",
        "T" => "Tackle",
        "G" => "Guard",
        "C" => "Center",
        "DE" => "Defensive end",
        "DT" => "Defensive tackle",
        "NT" => "Nose tackle",
        "OLB" => "Outside linebacker",
        "ILB" => "Inside linebacker",
        "MLB" => "Middle linebacker",
        "LB" => "Linebacker",
        "CB" => "Cornerback",
        "SS" => "Strong safety",
        "FS" => "Free safety",
        "DB" => "Defensive back",
        "LS" => "Long snapper",
        _ => "",
    }
}

pub fn role(pff_role: &str) -> &'static str {
    match pff_role {
        "Pass" => "Passer",
        "Pass Block" => "Pass block",
        "Pass Route" => "Route",
        "Pass Rush" => "Pass rush",
        "Coverage" => "Coverage",
        _ => "Unknown",
    }
}

/// `1141` → `1,141`.
pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn labels() {
        assert_eq!(pass_result("IN"), "Interception");
        assert_eq!(down(2), "2nd");
        assert_eq!(quarter(5), "OT");
        assert_eq!(title_case("DESIGNED_ROLLOUT_RIGHT"), "Designed rollout right");
        assert_eq!(thousands(1141), "1,141");
        assert_eq!(thousands(12), "12");
        assert_eq!(coverage_kind("Cover-3"), "Zone");
    }
}
