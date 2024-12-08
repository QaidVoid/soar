pub mod core {
    use std::path::Path;

    use refinery::embed_migrations;
    use rusqlite::Connection;

    embed_migrations!("migrations/core");

    pub fn init_db<P: AsRef<Path>>(path: P) -> Result<(), Box<dyn std::error::Error>> {
        let path = path.as_ref();
        let mut conn = Connection::open(path)?;
        migrations::runner().run(&mut conn)?;
        Ok(())
    }
}

pub mod metadata {
    use std::path::Path;

    use refinery::embed_migrations;
    use rusqlite::Connection;

    embed_migrations!("migrations/metadata");

    pub fn init_db<P: AsRef<Path>>(path: P) -> Result<(), Box<dyn std::error::Error>> {
        let path = path.as_ref();
        let mut conn = Connection::open(path)?;
        migrations::runner().run(&mut conn)?;
        Ok(())
    }
}
