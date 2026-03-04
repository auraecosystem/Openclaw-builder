# Evolutionary & ML-Based Parameter Optimization

Meta-optimization layer that sits above individual indicators: finds optimal indicator parameters (periods, thresholds, weights, compositions) by treating backtested signal quality as a black-box fitness function. Covers six families from 2025-2026 literature, ordered from simplest-to-replace-CMA-ES to most architecturally novel.

## Problem Framing

Given a parameterized indicator `I(θ)` and a fitness function `f(θ) → ℝ` (e.g., Sharpe ratio on walk-forward windows), find:

```
θ* = argmax f(θ),  θ ∈ Θ
```

where `Θ` may be continuous (period lengths, tolerances), discrete (indicator type, kernel), or mixed. `f` is:
- **Expensive**: each evaluation = full backtest pass
- **Noisy**: same `θ` gives different `f` across time windows
- **Non-convex**: multiple local optima across regimes
- **Black-box**: no gradient available (most backtesting pipelines)

---

## CMA-ES (Current Baseline)

Covariance Matrix Adaptation Evolution Strategy. Maintains a multivariate Gaussian `N(m, σ²C)` over the search space and adapts both mean `m` and full covariance `C` from selected offspring.

### Algorithm

```
Initialize: m ∈ ℝⁿ, σ ∈ ℝ, C = I, p_σ = 0, p_c = 0

Each generation:
1. Sample λ offspring:
   x_k ~ m + σ · N(0, C),  k = 1..λ

2. Evaluate: f_k = f(x_k)

3. Select μ < λ best by f; compute weights w_i (sum to 1)

4. Update mean:
   m ← Σᵢ w_i · x_{i:λ}

5. Cumulative step-size path (CSA):
   p_σ ← (1-c_σ)·p_σ + √(c_σ(2-c_σ)μ_w) · C^{-½} · (m_new - m_old)/σ
   σ ← σ · exp(c_σ/d_σ · (‖p_σ‖/E‖N(0,I)‖ - 1))

6. Rank-1 + rank-μ covariance update:
   p_c ← (1-c_c)·p_c + h_σ · √(c_c(2-c_c)μ_w) · (m_new - m_old)/σ
   C ← (1-c_1-c_μ)·C + c_1·p_c·p_cᵀ + c_μ·Σᵢ w_i·(x_{i:λ}-m_old)(x_{i:λ}-m_old)ᵀ/σ²
```

**Key properties:**
- Invariant to rotation and scaling of search space
- ~3n + 70 function evaluations to convergence on smooth unimodal functions
- Scales to ~100–200 dimensions before covariance matrix becomes unwieldy

### 2025-2026 Variants

**CMA-ES with Learning Rate Adaptation** (Nomura & Ono, ACM TOELO 2024): eliminates sensitivity to initial σ by adapting the learning rates c₁, c_μ online. Removes the last major hand-tuned hyperparameter.

**Warm-Starting CMA-ES** (PPSN 2024): seeds `m` and `C` from a prior run on a related problem (e.g., adjacent walk-forward window). Reduces evaluations by 30-50% when objectives are locally stationary.

### Parameters

| Parameter | Symbol | Typical Default | Description |
|---|---|---|---|
| Population | `λ` | `4 + floor(3·ln(n))` | Offspring per generation |
| Parents | `μ` | `λ/2` | Selected for update |
| Step size | `σ` | 0.3 × range | Initial distribution width |
| Tolerance | `tol_f` | 1e-11 | Convergence criterion |
| Max evals | `maxiter` | `100·n²` | Budget |

### Limitations
- Scales poorly beyond ~200 dimensions (covariance matrix O(n²) storage)
- Stagnates when fitness landscape is multi-modal with widely separated basins
- Noisy fitness (stochastic backtests) requires increased λ or fitness averaging

---

## Differential Evolution / JADE

Population-based optimizer that constructs trial vectors via differential mutation. JADE is the adaptive variant with external archive; consistently competitive with CMA-ES on high-dimensional noisy landscapes.

### Algorithm (JADE)

```
Initialize: population P = {x₁, ..., x_NP}, each xᵢ ∈ ℝⁿ
            archive A = {} (failed parents)
            μ_F = 0.5, μ_CR = 0.5  (adaptive parameter means)

Each generation:
1. For each target xᵢ:

   a. Sample Fᵢ ~ Cauchy(μ_F, 0.1), clip to (0, 1]
      Sample CRᵢ ~ N(μ_CR, 0.1), clip to [0, 1]

   b. DE/current-to-pbest/1 mutation:
      x_best ~ top p% of population (p ∈ [0.05, 0.2])
      r1 ≠ r2 ∉ {i}, r1 from P, r2 from P ∪ A
      v = xᵢ + Fᵢ·(x_best - xᵢ) + Fᵢ·(x_r1 - x_r2)

   c. Binomial crossover:
      u_j = v_j  if rand() ≤ CRᵢ or j = rand_dim
            x_j  otherwise

   d. Selection: xᵢ ← u if f(u) ≤ f(xᵢ), else add xᵢ to A

2. Trim A to size NP (remove randomly)

3. Update adaptive parameters from successful Fᵢ, CRᵢ:
   μ_F ← (1-c)·μ_F + c·mean_L(S_F)    # Lehmer mean
   μ_CR ← (1-c)·μ_CR + c·mean(S_CR)
```

**Why better than vanilla DE for trading:**
- Current-to-pbest mutation exploits elite solutions (directional guidance)
- Cauchy-distributed F allows occasional large jumps over fitness barriers
- Archive retains diversity from eliminated parents
- Parameter self-adaptation removes manual F/CR tuning

### Parameters

| Parameter | Symbol | Default | Description |
|---|---|---|---|
| Population | `NP` | `10·n` | Population size |
| pbest fraction | `p` | 0.05–0.20 | Elite fraction for mutation |
| Archive size | `|A|` | `NP` | Failed-parent diversity pool |
| Adaptation rate | `c` | 0.1 | μ_F, μ_CR update speed |
| Max generations | — | 500–2000 | Budget |

### Complexity
O(NP · n) per generation, O(NP · n) memory.

### References
Zhang & Sanderson (2009) IEEE TEVC. ["Adaptive Differential Evolution with Optional External Archive"](https://ieeexplore.ieee.org/document/4632146)

---

## Multi-Objective Evolutionary Optimization

Optimizes multiple conflicting fitness criteria simultaneously, yielding a **Pareto front** of non-dominated solutions rather than a single point. Natural fit for trading: Sharpe vs Max Drawdown, Return vs Turnover.

### NSGA-II Algorithm

```
Initialize: P₀ of size N, evaluate objectives f₁(x), f₂(x), ...

Each generation:
1. Create offspring Qₜ via binary tournament + SBX crossover + polynomial mutation
2. Combine Rₜ = Pₜ ∪ Qₜ (size 2N)
3. Non-dominated sort → fronts F₁, F₂, ..., F_k where:
   x dominates y iff f_j(x) ≤ f_j(y) ∀j and ∃j: f_j(x) < f_j(y)
4. Select next population: add fronts until |P_{t+1}| = N
   Partial front: rank by crowding distance (preserves diversity)
5. Crowding distance for solution i in objective j:
   d_i += (f_j[i+1] - f_j[i-1]) / (f_j^max - f_j^min)
```

**Practical objectives for precursor detection:**
- `f₁ = -Sharpe`  (maximize)
- `f₂ = MaxDrawdown`  (minimize)
- `f₃ = -Calmar = MaxDD / AnnReturn`  (minimize)
- `f₄ = 1/Stability`  (minimize variance of rolling Sharpe)

### MOEA/D (Decomposition-Based)

Decomposes the multi-objective problem into N scalar subproblems via weight vectors `λ¹, ..., λᴺ`.

```
For each subproblem i with weight λⁱ:
  Neighborhood B(i) = k closest weight vectors by Euclidean distance

Update (Tchebycheff scalarization):
  g(x|λ,z*) = max_j { λⱼ · |f_j(x) - z*_j| }
  where z* = ideal point (best known f_j value)

Each generation: for each i, pick parent from B(i), generate child,
  update neighbors if child improves their g(·)
```

**MOEA/D vs NSGA-II:**
- MOEA/D significantly faster on complex Pareto set shapes
- NSGA-II more robust when weight vector placement is hard to specify
- For 2 objectives: NSGA-II is standard; 3+ objectives: MOEA/D preferred

### AGE-MOEA2 (2025 State-of-Art)

Replaces crowding distance with survival score based on Pareto front curvature, yielding better-spread fronts without specifying a reference point. Per Springer Computational Economics (2025): outperforms NSGA-II and MOEA/D on combined Sharpe/DD trading objectives.

### Parameters

| Parameter | Symbol | Typical | Description |
|---|---|---|---|
| Population | `N` | 100–300 | Must be even |
| Crossover prob | `p_c` | 0.9 | SBX crossover rate |
| Mutation prob | `p_m` | 1/n | Polynomial mutation rate |
| Distribution idx | `η_c, η_m` | 20, 20 | Controls offspring spread |
| Neighbors (MOEA/D) | `k` | 20 | Neighborhood size |

### References
- Deb et al. (2002) ["A Fast and Elitist Multiobjective Genetic Algorithm: NSGA-II"](https://ieeexplore.ieee.org/document/996017)
- Zhang & Li (2007) ["MOEA/D: A Multiobjective Evolutionary Algorithm Based on Decomposition"](https://ieeexplore.ieee.org/document/4358754)
- Panichella (2019) ["An Adaptive Evolutionary Algorithm Based on Non-Euclidean Geometry for Many-Objective Optimization (AGE-MOEA)"](https://dl.acm.org/doi/10.1145/3321707.3321839)
- ["Optimal Technical Indicator Trading via MOEA"](https://link.springer.com/article/10.1007/s10614-024-10701-6) Computational Economics (2025)
- ["Multi-objective GP with directional changes + modified Sharpe"](https://link.springer.com/article/10.1007/s10462-025-11390-9) AI Review (2025)

---

## Grammar-Guided Genetic Programming (G3P)

Discovers new indicator *formulas* rather than tuning parameters of existing ones. Represents the search space as a context-free grammar over financial operators; evolves derivation trees.

### Algorithm

```
Grammar G = (N, T, P, S):
  Non-terminals N: {expr, binary_op, unary_op, window_fn, scalar}
  Terminals T: {close, volume, +, -, *, /, abs, log, ts_mean(·,n), ts_std(·,n),
                ts_rank(·,n), delay(·,k), n ∈ {5,10,20,60}, k ∈ {1,2,5}}
  Productions P: expr → binary_op(expr, expr) | unary_op(expr) | window_fn(expr, n) | scalar
  Start symbol S: expr

Population: parse trees derived from G
  Each tree encodes one alpha formula, e.g.:
  ts_std(log(close / delay(close, 1)), 20) / ts_mean(abs(close - delay(close,5)), 10)

Genetic operators:
  Crossover: swap subtrees at random matching non-terminals
  Mutation: re-derive a subtree from a random non-terminal
  Selection: tournament selection on IC (information coefficient) or Sharpe

Fitness: f(tree) = IC(alpha_values, forward_returns) over in-sample window
```

**AlphaCFG (2026):** Adds financial validity constraints to grammar:
- Dimension-type system: prevents price/volume nonsense combos
- Complexity penalty: prefers shorter trees
- Result: grammatically valid + financially interpretable alphas

### Practical Grammar for Precursor Indicators

```
indicator → entropy_fn(price_series, period)
           | changepoint_fn(price_series, period, threshold)
           | trend_fn(price_series, period)
           | ratio(indicator, indicator)
           | smooth(indicator, ema_period)

entropy_fn → PermutationEntropy | ApproximateEntropy | SampleEntropy
changepoint_fn → BOCPD | CUSUM | PELT
trend_fn → KalmanTrend | L1Trend | EMD_IMF(k)
period → {20, 50, 100, 200, 500}
```

**Why useful:** finds non-obvious indicator compositions that a human wouldn't design. AlphaCFG (2026) demonstrated on US and Chinese market data: grammar-discovered alphas outperformed all handcrafted baselines on IC and profitability.

### Parameters

| Parameter | Description | Typical |
|---|---|---|
| Population | Trees per generation | 200–1000 |
| Max tree depth | Prevents bloat | 6–8 |
| Tournament size | Selection pressure | 3–7 |
| Crossover prob | Subtree swap rate | 0.9 |
| Mutation prob | Subtree re-derive | 0.1 |
| Parsimony coeff | Penalty per node | 0.001–0.01 |

### Complexity
O(pop × depth × n_bars) per generation. Embarrassingly parallelizable per individual.

### References
- ["Alpha Discovery via Grammar-Guided Learning and Search" (AlphaCFG, 2026)](https://arxiv.org/html/2601.22119)
- ["Evolving Financial Trading Strategies with Vectorial GP" (2025)](https://arxiv.org/html/2504.05418v1)
- ["Multi-objective GP with Directional Changes + Sharpe" (2025)](https://link.springer.com/article/10.1007/s10462-025-11390-9)

---

## Surrogate-Assisted Optimization (Bayesian + EA Hybrid)

When each fitness evaluation is expensive (walk-forward backtest takes seconds to minutes), a surrogate model approximates `f(θ)` cheaply, directing the expensive EA budget toward promising regions.

### Algorithm (B²EA: Bayesian-Boosted EA)

```
Initialize: evaluate f(θ) at N₀ initial points (LHS sampling)
            build Gaussian Process surrogate: GP(m(θ), k(θ,θ'))

Each iteration:
1. Acquisition step (Bayesian):
   a. Fit GP to all {θ_i, f_i} observations
   b. Compute Expected Improvement (EI) or Upper Confidence Bound (UCB):
      EI(θ) = E[max(f(θ) - f*, 0)]
             = (μ(θ) - f*)·Φ(Z) + σ(θ)·φ(Z),  Z = (μ(θ)-f*)/σ(θ)
      UCB(θ) = μ(θ) + κ·σ(θ)   (κ controls exploration/exploitation)
   c. Propose k candidates θ_acq = argmax EI(θ)  [cheap optimization of surrogate]

2. EA step:
   a. Use {θ_acq} as seeds + current EA population
   b. Run EA for g generations using surrogate f̂(θ) as fitness
   c. Select promising candidates from EA run

3. Expensive evaluation: compute f(θ_candidate) for top m candidates
4. Update surrogate: add {θ_candidate, f(θ_candidate)} to dataset
5. Terminate when budget exhausted or convergence

Expected savings: 5-10x fewer real evaluations vs pure EA
```

**Alternative surrogates:**
- **Random Forest**: handles categorical parameters (indicator type), no smoothness assumption
- **TPE (Tree-structured Parzen Estimator)**: Optuna's default; models `p(θ|good)` and `p(θ|bad)` separately, picks `argmax p(θ|good)/p(θ|bad)`
- **Neural Network surrogate**: faster for high-dimensional θ when enough data

### When to Use

| Scenario | Use Surrogate? |
|---|---|
| Backtest < 1 second | No — EA alone is faster |
| Backtest 1–60 seconds, n < 20 dims | Yes (GP surrogate) |
| Backtest 1–60 seconds, n > 20 dims | Yes (RF or TPE surrogate) |
| Categorical + continuous mixed space | Yes (TPE) |

### Parameters

| Parameter | Description | Typical |
|---|---|---|
| Initial samples `N₀` | LHS exploration budget | 5·n |
| Acquisition | EI, UCB, PI | EI |
| UCB kappa `κ` | Exploration weight | 2.0 |
| Surrogate refit | Every N evals | 5–20 |
| EA generations per iter | Inner EA budget | 10–50 |

### References
- ["Hyperparameter Optimization in Machine Learning" survey (2025)](https://arxiv.org/html/2410.22854v3)
- B²EA: ["Combining Evolutionary Algorithms with Bayesian Optimization"](https://www.nature.com/articles/s41598-023-32027-3) Sci. Reports (2023)
- Bergstra & Bengio (2012) "Random Search for Hyper-parameter Optimization" (TPE foundations)

---

## Quality-Diversity Optimization (MAP-Elites / CMA-MAE)

Instead of a single optimal strategy, builds an **archive of high-quality, behaviorally diverse strategies**. Natural for trading: maintains a Pareto-like repertoire of regime-specialized indicators without explicitly defining objectives for each regime.

### MAP-Elites Algorithm

```
Define behavior descriptor space B = {b₁, b₂} (e.g., trend_responsiveness × volatility_sensitivity)
Discretize B into G cells (e.g., 20×20 = 400 cells)
Initialize archive: empty map cell → (θ, fitness)

Each iteration:
1. Select parent:
   a. If archive has < 20% cells filled: sample θ from prior (exploration)
   b. Else: sample θ from archive (exploitation)

2. Generate candidate θ' via mutation (CMA-ES step or DE/1/rand)

3. Evaluate:
   a. f(θ') = fitness (Sharpe on current window)
   b. b(θ') = behavior descriptor (computed from strategy properties)
      e.g.: b₁ = fraction of returns from trending bars
            b₂ = mean holding period in high-vol regime

4. Find cell c = discretize(b(θ'))
5. If archive[c] is empty or f(θ') > archive[c].fitness:
   archive[c] ← (θ', f(θ'))

Output: filled archive = repertoire of strategies, one per behavioral niche
```

### CMA-MAE (2024 State-of-Art QD)

CMA-MAE (Covariance Matrix Adaptation MAP-Annealing) fixes instability in CMA-ME by annealing the learning rate based on improvement fraction:

```
Learning rate annealing:
  α = 1 if improvement_frac ≥ min_frac
  α = improvement_frac / min_frac else  (slow down when barely improving)

CMA update uses α to scale covariance adaptation:
  C ← (1 - α·(c₁+c_μ))·C + α·(c₁·rank1_term + c_μ·rank_μ_term)
```

Achieves state-of-the-art on all QD benchmarks (QD-score: area under filled-archive-fitness curve).

### Behavior Descriptors for Trading Indicators

Designing the behavior space is the key art:

| Descriptor | Computation | Why useful |
|---|---|---|
| Trend signal fraction | % bars where Sharpe improves in trending regime | Finds trend-following vs mean-reversion specialists |
| Volatility sensitivity | Corr(indicator value, realized vol) | Separates vol-regime specialists |
| Mean holding period | Avg bars between signal flips | Frequency specialist (HFT vs swing) |
| Drawdown profile | Skewness of return distribution | Risk character classification |
| Crisis alpha | Sharpe during tail-event windows | Tail-hedge vs carry specialists |

### Output and Usage

The archive (filled map) is the artifact:
- Select the cell matching current regime: `regime_detector → b → archive[cell] → θ`
- Ensemble: weighted average across nearby cells
- Periodic re-archive as market regimes drift

### Parameters

| Parameter | Description | Typical |
|---|---|---|
| Grid resolution | Cells per behavioral dimension | 10–50 |
| Behavior dims | Number of descriptors | 2–4 |
| Archive capacity | Total cells | 100–10000 |
| Budget | Total evaluations | 10K–1M |
| CMA restart threshold | Min improvement fraction | 0.1 |

### Complexity
O(budget × eval_cost) evaluations, O(grid_cells × n_params) memory.

### References
- Mouret & Clune (2015) ["Illuminating Search Spaces by Mapping Elites"](https://arxiv.org/abs/1504.04909)
- Fontaine & Nikolaidis (2023) "Covariance Matrix Adaptation MAP-Annealing (CMA-MAE)"
- [Quality-Diversity Papers List](https://quality-diversity.github.io/papers.html)
- ["Rainbow Teaming" (QD for adversarial search, 2024)](https://arxiv.org/abs/2402.16822) — demonstrates QD on non-robotics domains

---

## Hybrid Gradient + EA

For differentiable fitness functions (e.g., when the backtest is approximated by a differentiable simulator or when optimizing neural indicator weights), combining gradient and evolutionary search avoids local optima while converging faster than pure EA.

### JADEGBO Pattern

```
Each generation:
1. DE mutation: v = xᵢ + F·(x_pbest - xᵢ) + F·(x_r1 - x_r2)
2. GBO local step:
   gradient estimate: g = (f(v + δ) - f(v - δ)) / (2δ)  [finite diff or autograd]
   Newton step: x_newton = v - (f(v+δ) - 2f(v) + f(v-δ))/δ² × g
3. Combine: u = rand·v + (1-rand)·x_newton
4. Selection: xᵢ ← u if f(u) > f(xᵢ)
```

**When applicable to trading:**
- Indicator weights in a linear combination: `signal = Σᵢ wᵢ · indicator_i(θᵢ)` — differentiable w.r.t. wᵢ
- Smooth threshold parameters: gradient available via finite differences
- Neural surrogate fitness: full autograd

---

## Comparison Summary

| Method | Best For | Eval Budget | Parallelizable | Multi-Obj |
|---|---|---|---|---|
| CMA-ES | Continuous, low-dim, smooth | Low (100-1K) | Moderate | No |
| JADE/DE | Continuous, high-dim, noisy | Medium (1K-10K) | High | No |
| NSGA-II | 2-3 objectives, moderate dim | Medium | High | Yes |
| MOEA/D | 3+ objectives, complex Pareto | Medium | High | Yes |
| G3P | Discover new formulas | High (10K+) | High | Optional |
| Surrogate+EA | Expensive evaluations | Low (50-500) | Low | Possible |
| MAP-Elites | Regime-diverse portfolio | High (100K+) | High | Implicit |
| Hybrid Gradient | Differentiable components | Low | Moderate | No |

## Implementation Status
Not yet implemented in engine or NT. These are meta-optimization methods that wrap the existing indicator/fitness infrastructure. Candidate implementation: extend `evolution/` module in the algotrader engine with JADE, NSGA-II, and MAP-Elites as pluggable optimizer backends alongside CMA-ES.
