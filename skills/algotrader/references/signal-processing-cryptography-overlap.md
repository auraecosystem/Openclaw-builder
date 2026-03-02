# Signal Processing ↔ Cryptography: Overlap Map

Both domains are fundamentally about extracting hidden structure from apparently random data. A noisy timeseries and an encrypted message present the same mathematical challenge — there's a signal buried under something that obscures it (noise vs. encryption), and the goal is to recover the underlying pattern.

Shannon recognized this explicitly: his two foundational papers treated communication-through-noise and communication-through-encryption as dual problems. The tools that separate signal from noise (Fourier, wavelets, HMMs, Kalman, SVD) also separate plaintext from ciphertext when the encryption is imperfect.

---

## Deep Overlaps (Direct Application in Both Domains)

### Frequency Analysis / Fourier Transform
The most fundamental crossover. In signal processing, FFT decomposes signals into frequency components. In cryptanalysis, frequency analysis of letter/symbol distributions has been the primary codebreaking technique since the 9th century (Al-Kindi). Modern application: spectral attacks on block ciphers, and the Walsh-Hadamard Transform is literally a Fourier transform of the cipher's differential distribution table.

### Hidden Markov Models
Extensively used in both domains. In timeseries: regime detection, pattern recognition. In cryptanalysis: breaking substitution ciphers by modeling the hidden plaintext as latent states emitting observed ciphertext symbols. Also used for side-channel attacks — modeling randomized masking as hidden states, recovering secret keys from as few as 10 power traces. Additionally applied to encryption algorithm classification.

### Autocorrelation / Periodicity Detection
Direct overlap. In timeseries: detect repeating patterns at various lags. In cryptanalysis: correlation attacks on stream ciphers (Siegenthaler correlation attack). If the keystream has detectable autocorrelation (probability > 0.5 correlation with an individual LFSR), the cipher can be broken.

### Information Theory / Entropy
Shannon's 1948 "Mathematical Theory of Communication" founded information theory for signal processing; his 1949 "Communication Theory of Secrecy Systems" founded mathematical cryptography. Entropy measures randomness in both: signal-to-noise ratio in timeseries, and key space unpredictability in crypto. Rényi entropy specifically evaluates randomness in cryptographic systems.

### Kalman Filter
In timeseries: recursive state estimation under noise. In cryptanalysis: side-channel attacks — fusing power consumption and electromagnetic measurements to reduce noise and extract secret keys faster than traditional statistical methods. Kalman-filtered side-channel attacks significantly reduce the number of traces needed for successful key recovery.

### SVD / Matrix Decomposition
In timeseries: SSA uses SVD to separate signal from noise. In cryptanalysis: directly applicable to breaking chaotic encryption schemes. Lattice reduction algorithms (LLL, BKZ) used in lattice-based cryptography are fundamentally matrix decomposition methods.

### Neural Networks / Deep Learning
In timeseries: autoencoders, transformers for pattern extraction. In cryptanalysis: deep learning attacks on block ciphers (DES, AES, SPECK), side-channel analysis with higher accuracy than traditional methods, and differential fault analysis.

---

## Moderate Overlaps (Shared Mathematical Framework)

### Wavelet Transform
In timeseries: multi-resolution denoising. In cryptography: wavelets are used for image encryption (decompose into subbands, encrypt selectively) and watermarking. Also used in side-channel attacks for denoising power traces before analysis.

### Bayesian / Statistical Inference
In timeseries: Gaussian processes, BSTS. In cryptanalysis: statistical attacks on LWE-based post-quantum cryptography by exploiting decryption error leakage. Statistical methods underpin differential and linear cryptanalysis.

### Change-Point Detection
In timeseries: detect structural breaks. In cryptanalysis: analogous to detecting where a cipher key changes in a polyalphabetic cipher, or identifying boundaries between encrypted and unencrypted segments.

---

## Sources

- https://en.wikipedia.org/wiki/Frequency_analysis
- https://eprint.iacr.org/2019/256.pdf
- https://scholarworks.sjsu.edu/etd_projects/407/
- https://link.springer.com/chapter/10.1007/978-3-540-45238-6_3
- https://www.ieor.iitb.ac.in/files/seminar/Cryptanalysis.pdf
- https://pmc.ncbi.nlm.nih.gov/articles/PMC8870987/
- https://ieeexplore.ieee.org/document/5495428/
- https://www.academia.edu/1366189/A_Nonlinear_Generalization_of_Singular_Value_Decomposition_and_Its_Applications_to_Mathematical_Modeling_and_Chaotic_Cryptanalysis
- https://eprint.iacr.org/2023/098
- https://eprint.iacr.org/2024/1300.pdf
- https://eprint.iacr.org/2016/921.pdf
- https://matheo.uliege.be/bitstream/2268.2/6978/4/Memoire_Etienne_Elodie.pdf
