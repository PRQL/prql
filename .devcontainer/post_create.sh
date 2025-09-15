#! /bin/sh
#
# Other postCreateCommands that should only run in the Dev Container environment
# Since these are called by the .devcontainer/devcontainer.json's `postCreateCommand`
# they won't affect people building the software natively

git config --global --add safe.directory /workspaces/prql

pip install pre-commit
