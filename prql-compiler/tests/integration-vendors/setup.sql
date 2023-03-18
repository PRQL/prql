DROP TABLE IF EXISTS cars;


DROP TABLE IF EXISTS customers;


CREATE TABLE customers
(
    id integer NOT NULL,
    name varchar(255) NOT NULL,
    rich boolean NOT NULL,
    PRIMARY KEY (id)
);


CREATE TABLE cars
(
    id integer NOT NULL,
    customer integer NOT NULL,
    name varchar(255),
    PRIMARY KEY (id),
    CONSTRAINT c
        FOREIGN KEY (customer)
        REFERENCES customers (id)
);


INSERT INTO customers
(
    id,
    name,
    rich
)
VALUES
(0, 'Tony Stark', TRUE),
(1, 'Bruce Wayne', TRUE),
(2, 'Wade Wilson', FALSE);


INSERT INTO cars
(
    id,
    customer,
    name
)
VALUES
(0, 1, 'Mercedes'),
(1, 1, 'Porsche'),
(2, 0, 'Bugatti'),
(3, 0, 'Ferrari'),
(4, 0, 'Lamborghini'),
(5, 2, 'Toyota');