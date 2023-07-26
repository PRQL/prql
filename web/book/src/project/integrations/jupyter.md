# Jupyter

[pyprql](https://pypi.org/project/pyprql/) contains `pyprql.magic`, a thin
wrapper of [`JupySQL`](https://pypi.org/project/jupysql/)'s SQL IPython magics.
This allows us to run PRQL interactively on Jupyter/IPython.

Check out <https://pyprql.readthedocs.io/> for more context.

## Installation

```sh
pip install pyprql
```

## Usage

When installing pyprql, the
[duckdb-engine](https://pypi.org/project/duckdb-engine/) package is also
installed with it, so we can start using PRQL immediately to query CSV and
Parquet files.

For example, running
[the example from the JupySQL documentation](https://jupysql.ploomber.io/en/latest/quick-start.html)
on IPython:

```python
In [1]: %load_ext pyprql.magic

In [2]: !curl -sL https://raw.githubusercontent.com/mwaskom/seaborn-data/master/penguins.csv -o penguins.csv

In [3]: %prql duckdb://

In [4]: %prql from `penguins.csv` | take 3
Out[4]:
  species     island  bill_length_mm  bill_depth_mm  flipper_length_mm  body_mass_g     sex
0  Adelie  Torgersen            39.1           18.7                181         3750    MALE
1  Adelie  Torgersen            39.5           17.4                186         3800  FEMALE
2  Adelie  Torgersen            40.3           18.0                195         3250  FEMALE

In [5]: %%prql
   ...: from `penguins.csv`
   ...: filter bill_length_mm > 40
   ...: take 3
   ...:
   ...:
Out[5]:
  species     island  bill_length_mm  bill_depth_mm  flipper_length_mm  body_mass_g     sex
0  Adelie  Torgersen            40.3           18.0                195         3250  FEMALE
1  Adelie  Torgersen            42.0           20.2                190         4250    None
2  Adelie  Torgersen            41.1           17.6                182         3200  FEMALE
```
