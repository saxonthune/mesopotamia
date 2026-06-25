use crate::elk::Elk;

pub struct Metric {
    pub name: &'static str,
    pub extract: fn(&Elk) -> f32,
}

pub const ELK_METRICS: &[Metric] = &[
    Metric { name: "energy", extract: |e| e.energy },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn make_elk(energy: f32) -> Elk {
        Elk {
            cell: 0,
            prev_cell: 0,
            move_t: 1.0,
            move_rate: 0.0,
            slot: 0,
            code: 0,
            energy,
            digesting: vec![],
            grazing: false,
            at_edge: 0,
            intake_rate: 0.0,
            traveling: false,
        }
    }

    #[test]
    fn energy_extractor_returns_elk_energy() {
        let elk = make_elk(0.75);
        let m = ELK_METRICS.iter().find(|m| m.name == "energy").unwrap();
        assert_eq!((m.extract)(&elk), 0.75);
    }

    #[test]
    fn energy_extractor_zero() {
        let elk = make_elk(0.0);
        let m = ELK_METRICS.iter().find(|m| m.name == "energy").unwrap();
        assert_eq!((m.extract)(&elk), 0.0);
    }
}
