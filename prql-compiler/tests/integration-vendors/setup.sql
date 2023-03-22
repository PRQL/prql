DROP TABLE IF EXISTS cars;
DROP TABLE IF EXISTS people;
DROP TABLE IF EXISTS "Upper";

CREATE TABLE people
(
    id integer NOT NULL,
    name varchar(255) NOT NULL,
    rich boolean NOT NULL,
    PRIMARY KEY (id)
);

CREATE TABLE cars
(
    id integer NOT NULL,
    person integer NOT NULL,
    name varchar(255),
    price integer NOT NULL,
    PRIMARY KEY (id),
    CONSTRAINT c
        FOREIGN KEY (person)
        REFERENCES people (id)
);

CREATE TABLE "Upper"
(
    id integer NOT NULL
);

INSERT INTO people
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
    person,
    name,
    price
)
VALUES
(0, 1, 'Mercedes', 60000),
(1, 1, 'Porsche', 90000),
(2, 0, 'Bugatti', 400000),
(3, 0, 'Ferrari', 500000),
(4, 0, 'Lamborghini', 200000),
(5, 2, 'Toyota', 10000);

INSERT INTO "Upper"
(
    id
)
VALUES
(999);
