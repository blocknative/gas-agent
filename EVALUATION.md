# Gas Agent Evaluation

Gas Agents submit gas price predictions for EVM networks to the [Gas Network](https://gas.network/) for evaluation. The *Evaluation Function* scores each agent to determine which is providing the best prediction relative to onchain truth. Since the comparison uses the actual onchain non-zero minimum gas price, the evalution is a lagging measurement in order to wait for the comparison block to arrive.

## Evaluation Criteria

The goal of Gas Network is to provide an accurate and reliable gas price prediction service for EVM networks. In order to be accurate, predictions must be as near as possible to the minimum gas price for the block. Predictions must also not be below the minimum for the block as that would cause a settlement delay until block pricing comes down to the predicted value. Finally, the source of predictions must be lively to keep the data reliably fresh.

This results in the following evaluation criteria:

1. **Inclusion Mean**: The mean of the inclusion rates of the agent's predictions in the evaluation window.
2. **Stability Around Inclusion Mean**: The standard deviation of the inclusion rates of the agent's predictions in the evaluation window.
3. **Overpayment Mean**: The mean of the overpayment rates of the agent's predictions in the evaluation window.
4. **Stability Around Overpayment**: The standard deviation of the overpayment rates of the agent's predictions in the evaluation window.
5. **Liveliness**: The consistency of the agent in delivery predictions in the evaluation window.

# Score Function

The score function is a weighted sum of utility functions, with each feature represented by its own utility function and corresponding weight.

$$
\text{TotalScore}= w_{0}*u_{0}(x_{0})+w_{1}*u_{1}(x_{1})+w_{2}*u_{2}(x_{2})+w_{3}*u_{3}(x_{3})+w_{4}*u_{4}(x_{4})
$$

Each utility function output is bounded from [0,1]. The sum of all weights equal to 1.  Thus the TotalScore is bounded from [0,1] A perfect score is 1. Higher is better.

|  | weight | utility function | raw input (with example) |
| --- | --- | --- | --- |
| inclusion mean | $w_0=0.5$ | $u_0()$ | $x_0=0.9$  |
| stability for inclusion | $w_1 = 0.15$ | $u_1() = e^{-\beta_1*x}$ where $\beta_1=3.2$ | $x_1 = 2.5$ |
| overpayment mean | $w_2=0.15$ | $u_2()=e^{-\beta_2*x}$ where $\beta_2=3.2$ | $x_2 = 1.2$ |
| stability for overpayment | $w_3=0.10$ | $u_3()=e^{-\beta_3*x}$ where $\beta_3=3.2$ | $x_3=3.2$ |
| liveliness | $w_4=0.10$ | $u_4()$ | $x_4=0.9$  |

Note that the utility functions $u_0()$ and $u_4()$ are redundant, as the inclusion mean is already constrained to the interval [0, 1].

# Score Calculation Breakdown

A block window is a range of blocks for which an estimate is guaranteed to land on chain. As an example, the block window for GasAgents Ethereum is 1 block. (next block prediction)

After a block window is closed the following metrics are calculated and inserted into a fixed sized memory array. (default size = 10)

- inclusion (1 if estimate ≥ on chain block window minimum else 0)
- overpayment error (estimate - on chain block window minimum)
- timestamp of estimate (used for liveliness)

Inclusion & Overpayment

Once the array is filled, the performance metrics are calculated.  For each subsequent block window, the oldest value in the memory array is replaced, and a new performance metric is calculated after the update.

- inclusion rate (number of included estimates / array size (default 10))
- average overpayment error (sum of errors/array size)

These performance metrics are inserted into a AgentHistory array. The score metrics are calculated on the most recent entries in the last t seconds (default t = 60)

- mean inclusion rate
- standard deviation of inclusion rate
- average overpayment averages
- standard deviation of overpayment averages

Liveliness

For each block window in the fixed-size memory array, a 1 or 0 is assigned depending on whether a prediction was submitted. The liveliness metric is then calculated by summing these values and dividing by the size of the memory array.

## Exponential Bounding Function

The following metrics are bounded from $[-\inf,\inf]$

- inclusion COV
- overpayment mean
- overpayment COV

The utility function transforms these values bounds to [0,1]

$u = exp(- \beta * x)$ where $\beta=3.2$

We selected  $\beta$  based on the following constraints.

$$
\begin{cases}u=1 \hspace{0.2em}\text{when}\hspace{0.2em}x=0\hspace{3.6em}(1)\\
u=0.2\hspace{0.2em}\text{when}\hspace{0.2em}x=0.5\hspace{2em}(2)\end{cases}
$$

$u$ represents the utility produced by the given metric.

Constraint (1) ✅

 $u = 1$ is the highest possible utility and occurs when

- **The average overpayment is 0**, which means that the estimated gas price exactly matched the true on-chain minimum. There was neither overestimation nor underestimation overall.
- **The coefficient of variation (CV) is 0**, which indicates that the **standard deviation is 0**. There is **no variation** in the measured values.

    For example, if the **standard deviation of inclusion** is 0, this means that all inclusion metrics (e.g., inclusion times or probabilities) are **identical** across the dataset.


→ satisfied as $u = exp(0)=1$

Constraint (2) ✅

 $u = 0.2$ indicates a low utility and occurs when

- **The average overpayment is 50% (0.5),** which means that the estimate gas price was 50% more expensive then the on-chain minimum.
- **The coefficient of variation (CV) is 0.5,** which indicates that the spread is half as large as the mean. While a CV of 0.5 typically indicates moderate variation, in our context we aim to reward agents with minimal or no variability in their metric performance.
