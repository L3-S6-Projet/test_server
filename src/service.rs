use db::models::{Occupancy, OccupancyType};

const CM_COEFF: f64 = 1.5;
const TD_COEFF: f64 = 1.0;
const TP_COEFF: f64 = 2.0 / 4.0;
const PROJET_COEFF: f64 = 1.0;
const ADMINISTRATION_COEFF: f64 = 1.0;
const EXTERNAL_COEFF: f64 = 0.0;

pub fn service_value(occupancies: &[&Occupancy]) -> f64 {
    let mut total = 0.0;

    for occupancy in occupancies {
        let coeff = coeff(occupancy);
        let elapsed = occupancy.end_datetime - occupancy.start_datetime;
        let elapsed_hours = elapsed / 3600;
        total += elapsed_hours as f64 * coeff;
    }

    total
}

fn coeff(occupancy: &Occupancy) -> f64 {
    use OccupancyType::*;

    match occupancy.occupancy_type {
        CM => CM_COEFF,
        TD => TD_COEFF,
        TP => TP_COEFF,
        Projet => PROJET_COEFF,
        Administration => ADMINISTRATION_COEFF,
        External => EXTERNAL_COEFF,
    }
}

pub struct Service {
    pub cm: u32,
    pub projet: u32,
    pub td: u32,
    pub tp: u32,
    pub administration: u32,
    pub external: u32,
}

impl Service {
    pub fn total(&self) -> u32 {
        self.cm + self.projet + self.td + self.tp + self.administration + self.external
    }
}

impl Default for Service {
    fn default() -> Self {
        Self {
            cm: 0,
            projet: 0,
            td: 0,
            tp: 0,
            administration: 0,
            external: 0,
        }
    }
}

pub fn count_hours(occupancies: &[&Occupancy]) -> Service {
    use OccupancyType::*;

    let mut service = Service::default();

    for occupancy in occupancies {
        let elapsed = occupancy.end_datetime - occupancy.start_datetime;
        let elapsed_hours = (elapsed / 3600) as u32;

        match &occupancy.occupancy_type {
            CM => service.cm += elapsed_hours,
            TD => service.td += elapsed_hours,
            TP => service.tp += elapsed_hours,
            Projet => service.projet += elapsed_hours,
            Administration => service.administration += elapsed_hours,
            External => service.external += elapsed_hours,
        }
    }

    service
}
