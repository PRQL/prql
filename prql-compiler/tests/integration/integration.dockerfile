FROM ubuntu

RUN mkdir -p /workspace/prql-compiler/tests/integration
WORKDIR /workspace/prql-compiler/tests/integration

# insert pre-built test binary & tests
COPY integration .
COPY queries/ queries/
COPY snapshots/ snapshots/

# also insert chinook database
COPY data/chinook/ data/chinook/

ENV 'INSTA_WORKSPACE_ROOT' '/workspace'

CMD ["/workspace/prql-compiler/tests/integration/integration"]