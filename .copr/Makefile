srpm:
	dnf install -y git
	git config --global --add safe.directory .
	COPR_SRPM=$(outdir) ./ci/source-archive.sh
	COPR_SRPM=$(outdir) ./ci/deploy.sh
