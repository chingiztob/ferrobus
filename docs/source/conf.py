# Configuration file for the Sphinx documentation builder.
#
# For the full list of built-in configuration values, see the documentation:
# https://www.sphinx-doc.org/en/master/usage/configuration.html

# -- Project information -----------------------------------------------------
# https://www.sphinx-doc.org/en/master/usage/configuration.html#project-information
import os
import sys
from datetime import date

sys.path.insert(0, os.path.abspath("../../"))


project = "Ferrobus"
copyright = f"2025-{date.today().year}, Chingiz Zhanarbaev"
author = "Chingiz Zhanarbaev"
release = "0.1.2"

# -- General configuration ---------------------------------------------------
# https://www.sphinx-doc.org/en/master/usage/configuration.html#general-configuration

extensions = [
    "sphinx.ext.autodoc",
    "sphinx.ext.autosummary",
    "sphinx.ext.autosectionlabel",
    "sphinx.ext.napoleon",
    "sphinx.ext.viewcode",
    "myst_nb",
]

templates_path = ["_templates"]
exclude_patterns = []
nb_execution_mode = "off"
autosummary_generate = True
autosummary_imported_members = True

# -- Options for HTML output -------------------------------------------------
# https://www.sphinx-doc.org/en/master/usage/configuration.html#options-for-html-output

html_theme = "sphinx_book_theme"
html_static_path = ["_static"]

html_theme_options = {
    "github_url": "https://github.com/chingiztob/ferrobus",
    "logo": {
        "image_light": "_static/ferrobus_logo.png",
        "image_dark": "_static/ferrobus_logo_dark.png",
    },
    "navigation_depth": 2,
    "secondary_sidebar_items": ["page-toc", "edit-this-page"],
}
