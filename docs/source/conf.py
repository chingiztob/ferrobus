# Configuration file for the Sphinx documentation builder.
#
# For the full list of built-in configuration values, see the documentation:
# https://www.sphinx-doc.org/en/master/usage/configuration.html

# -- Project information -----------------------------------------------------
# https://www.sphinx-doc.org/en/master/usage/configuration.html#project-information
import os
import sys
from datetime import date

from ferrobus import __version__

sys.path.insert(0, os.path.abspath("../../"))


project = "Ferrobus"
copyright = f"2025-{date.today().year}, Chingiz Zhanarbaev"
author = "Chingiz Zhanarbaev"
version = release = __version__

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
html_css_files = ["custom.css"]
html_favicon = "_static/favicon.ico"

html_theme_options = {
    "logo": {
        "image_light": "_static/logo_light.svg",
        "image_dark": "_static/logo_dark.svg",
    },
    "navigation_depth": 2,
    "show_toc_level": 2,
    "secondary_sidebar_items": ["page-toc", "edit-this-page"],
    "repository_provider": "github",
    "repository_url": "https://github.com/chingiztob/ferrobus",
    "use_repository_button": True,
    "pygments_dark_style": "monokai",
    "pygments_light_style": "tango",
}
