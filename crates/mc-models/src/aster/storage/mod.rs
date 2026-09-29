//! The storage structures: the Capacitor Bank and the Materials Vault. Each shows its
//! side's store (`gpu_consts::store`): fill pieces light as the store fills, and status
//! lamps go amber while it drains, red when it is dry and green when it is full.

pub(super) mod bank;
pub(super) mod vault;
