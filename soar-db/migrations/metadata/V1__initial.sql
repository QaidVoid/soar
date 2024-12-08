CREATE TABLE repository (
  name TEXT NOT NULL UNIQUE
);

CREATE TABLE collections (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE
);

CREATE TABLE families (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL
);

CREATE TABLE homepages (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  url TEXT NOT NULL,
  package_id INTEGER NOT NULL,
  FOREIGN KEY (package_id) REFERENCES packages (id),
  UNIQUE (url, package_id)
);

CREATE TABLE notes (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  note TEXT NOT NULL,
  package_id INTEGER NOT NULL,
  FOREIGN KEY (package_id) REFERENCES packages (id),
  UNIQUE (note, package_id)
);

CREATE TABLE source_urls (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  url TEXT NOT NULL,
  package_id INTEGER NOT NULL,
  FOREIGN KEY (package_id) REFERENCES packages (id),
  UNIQUE (url, package_id)
);

CREATE TABLE icons (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  url TEXT NOT NULL UNIQUE
);

CREATE TABLE provides (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  family_id INTEGER NOT NULL,
  package_id INTEGER NOT NULL,
  FOREIGN KEY (package_id) REFERENCES packages (id),
  FOREIGN KEY (family_id) REFERENCES families (id),
  UNIQUE (family_id, package_id)
);

CREATE TABLE packages (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  collection_id INTEGER NOT NULL,
  family_id INTEGER NOT NULL,
  icon_id INTEGER NOT NULL,
  pkg TEXT NOT NULL,
  pkg_id TEXT,
  pkg_name TEXT NOT NULL,
  app_id TEXT,
  description TEXT,
  version TEXT NOT NULL,
  download_url TEXT NOT NULL,
  size INTEGER NOT NULL,
  checksum TEXT NOT NULL,
  build_date TEXT NOT NULL,
  build_script TEXT NOT NULL,
  build_log TEXT NOT NULL,
  category TEXT,
  desktop TEXT,
  FOREIGN KEY (collection_id) REFERENCES collections (id),
  FOREIGN KEY (family_id) REFERENCES families (id),
  FOREIGN KEY (icon_id) REFERENCES icons (id),
  UNIQUE (family_id, pkg_name)
);
