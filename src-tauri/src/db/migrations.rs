use rusqlite_migration::{Migrations, M};

pub fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(include_str!("../../migrations/001_initial.sql")),
        M::up(include_str!("../../migrations/002_work_days.sql")),
        M::up(include_str!("../../migrations/003_task_sprint_flag.sql")),
    ])
}
