srpm:
	dnf install -y git
	COPR_SRPM=$(outdir) ./ci/source-archive.sh
	COPR_SRPM=$(outdir) ./ci/deploy.sh
