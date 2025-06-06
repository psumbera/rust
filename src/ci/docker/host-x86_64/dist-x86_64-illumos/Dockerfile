FROM ubuntu:18.04

# Enable source repositories, which are disabled by default on Ubuntu >= 18.04
RUN sed -i 's/^# deb-src/deb-src/' /etc/apt/sources.list

COPY scripts/cross-apt-packages.sh /tmp/
RUN bash /tmp/cross-apt-packages.sh

# Required for cross-build gcc, and we install python2 to test general compatibility.
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    libgmp-dev \
    libmpfr-dev \
    libmpc-dev \
    python2.7 \
    && rm -rf /var/lib/apt/lists/*

COPY scripts/shared.sh /tmp/
COPY scripts/illumos-toolchain.sh /tmp/

RUN bash /tmp/illumos-toolchain.sh x86_64 sysroot
RUN bash /tmp/illumos-toolchain.sh x86_64 binutils
RUN bash /tmp/illumos-toolchain.sh x86_64 gcc

COPY scripts/sccache.sh /scripts/
RUN sh /scripts/sccache.sh

COPY scripts/cmake.sh /scripts/
RUN /scripts/cmake.sh

ENV \
    AR_x86_64_unknown_illumos=x86_64-illumos-ar \
    RANLIB_x86_64_unknown_illumos=x86_64-illumos-ranlib \
    CC_x86_64_unknown_illumos=x86_64-illumos-gcc \
    CXX_x86_64_unknown_illumos=x86_64-illumos-g++

ENV HOSTS=x86_64-unknown-illumos

ENV RUST_CONFIGURE_ARGS --enable-extended --disable-docs
ENV SCRIPT python2.7 ../x.py dist --host $HOSTS --target $HOSTS
