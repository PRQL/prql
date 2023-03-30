DROP TABLE IF EXISTS invoices;
DROP TABLE IF EXISTS customers;
DROP TABLE IF EXISTS employees;
DROP TABLE IF EXISTS tracks;
DROP TABLE IF EXISTS albums;
DROP TABLE IF EXISTS genres;
DROP TABLE IF EXISTS playlist_track;
DROP TABLE IF EXISTS playlists;
DROP TABLE IF EXISTS media_types;
DROP TABLE IF EXISTS artists;
DROP TABLE IF EXISTS invoice_items;

CREATE TABLE invoices (
    invoice_id INTEGER,
    customer_id INTEGER,
    invoice_date TIMESTAMP,
    billing_address VARCHAR(255),
    billing_city VARCHAR(255),
    billing_state VARCHAR(255),
    billing_country VARCHAR(255),
    billing_postal_code VARCHAR(255),
    total REAL
);
CREATE TABLE customers (
    customer_id INTEGER,
    first_name VARCHAR(255),
    last_name VARCHAR(255),
    company VARCHAR(255),
    address VARCHAR(255),
    city VARCHAR(255),
    state VARCHAR(255),
    country VARCHAR(255),
    postal_code VARCHAR(255),
    phone VARCHAR(255),
    fax VARCHAR(255),
    email VARCHAR(255),
    support_rep_id INTEGER
);
CREATE TABLE employees (
    employee_id INTEGER,
    last_name VARCHAR(255),
    first_name VARCHAR(255),
    title VARCHAR(255),
    reports_to INTEGER,
    birth_date TIMESTAMP,
    hire_date TIMESTAMP,
    address VARCHAR(255),
    city VARCHAR(255),
    state VARCHAR(255),
    country VARCHAR(255),
    postal_code VARCHAR(255),
    phone VARCHAR(255),
    fax VARCHAR(255),
    email VARCHAR(255)
);
CREATE TABLE tracks (
    track_id INTEGER,
    name VARCHAR(255),
    album_id INTEGER,
    media_type_id INTEGER,
    genre_id INTEGER,
    composer VARCHAR(255),
    milliseconds INTEGER,
    bytes INTEGER,
    unit_price REAL
);
CREATE TABLE albums (album_id INTEGER, title VARCHAR(255), artist_id INTEGER);
CREATE TABLE genres (genre_id INTEGER, name VARCHAR(255));
CREATE TABLE playlist_track (playlist_id INTEGER, track_id INTEGER);
CREATE TABLE playlists (playlist_id INTEGER, name VARCHAR(255));
CREATE TABLE media_types (media_type_id INTEGER, name VARCHAR(255));
CREATE TABLE artists (artist_id INTEGER, name VARCHAR(255));
CREATE TABLE invoice_items (
    invoice_line_id INTEGER,
    invoice_id INTEGER,
    track_id INTEGER,
    unit_price REAL,
    quantity INTEGER
);
