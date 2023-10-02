srpm:
	dnf install -y git
	git config --global --add safe.directory $$PWD
	git config --global --add safe.directory $$PWD/deps/freetype/freetype2
	git config --global --add safe.directory $$PWD/deps/freetype/libpng
	git config --global --add safe.directory $$PWD/deps/freetype/zlib
	git config --global --add safe.directory $$PWD/deps/harfbuzz/harfbuzz
	COPR_SRPM=$(outdir) ./ci/source-archive.sh
	COPR_SRPM=$(outdir) ./ci/deploy.sh
