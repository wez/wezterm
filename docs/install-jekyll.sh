#!/bin/sh
gem install --user-install bundler jekyll
GEM_HOME=$HOME/.gem bundle install

echo "You can now use 'GEM_HOME=$HOME/.gem bundle exec jekyll serve' to test the docs locally"
