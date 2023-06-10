# Palette-based Image Recoloring

A Rust-based implementation of the recoloring technique from Efficient palette-based decomposition and recoloring of images via RGBXY-space geometry by Tan et. al. [link](https://cragl.cs.gmu.edu/fastlayers/Efficient%20palette-based%20decomposition%20and%20recoloring%20of%20images%20via%20RGBXY-space%20geometry%20(Jianchao%20Tan,%20Jose%20Echevarria,%20Yotam%20Gingold%202018%20SIGGRAPH%20Asia)%20600dpi.pdf).

The repository has the following parts:
* qhull-rs - Typesafe, Rust wrapper around the qhull C library. The API of this wrapper is heavily inspired by scipy.spatial, but only implements the minimum functionality for supporting the recoloring algorithm.
* qhull-sys - Low-level Rust wrapper around qhull. This exists to support qhull-rs. It compiles and links libqhull_r in addition to exposing the C-API.
* image-palette-recoloring - Rust library that implements the recoloring algorithm
* image-palette-recoloring-cli - Rust CLI program that allows one to try out the recoloring algorithm. Note that this will be much slower than ideal since it will have to recompute image weight information every time the program is run.
* image-palette-recoloring-c - C wrapper around palette-image-recoloring. See the included C header.
* image-palette-recoloring-web - basic HTML GUI to test out the algorithm. This relies on palette-image-recoloring-c complied to wasm. You can find a live version [here](https://aprilwade.github.io/image-palette-recoloring) .
