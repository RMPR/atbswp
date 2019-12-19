SHELL := bash
.PHONY: help prepare-dev test run
.ONESHELL:
.SHELLFLAGS := -eu -o pipefail -c
.DELETE_ON_ERROR:
MAKEFLAGS += --warn-undefined-variables
MAKEFLAGS += --no-builtin-rules

ifeq ($(origin .RECIPEPREFIX), undefined)
  $(error This Make does not support .RECIPEPREFIX. Please use GNU Make 4.0 or later)
endif
.RECIPEPREFIX = >

VENV_NAME?=.
VENV_ACTIVATE=$(VENV_NAME)/bin/activate
PYTHON=${VENV_NAME}/bin/python

.DEFAULT: help
help: ## Display this help section
> @awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z0-9_-]+:.*?## / {printf "\033[36m%-38s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)
.DEFAULT_GOAL := help

prepare-dev: ## Prepare the development environment, must only be used once
> python -m venv .
> ${PYTHON} -m pip install -r requirements-dev.txt

test: $(VENV_NAME)/bin/activate
> ${PYTHON} -m pytest

clean-pyc:  ## Clean all the pyc files
> find . -name '*.pyc' -exec rm --force {} +
> find . -name '*.pyo' -exec rm --force {} +
> name '*~' -exec rm --force  {}

clean-build:  ## Clean previous build
> rm --force --recursive build/
> rm --force --recursive dist/
> rm --force --recursive *.egg-info

build: $(VENV_NAME)/bin/activate ## Build the project for the current platform
> cd atbswp &&\
> pyinstaller atbswp.py && \
> cd ..

run:  ## Launch the project
> cd atbswp &&\
> python atbswp.py && \
> cd ..
