use std::marker::PhantomData;

use halo2wrong::halo2::{
    arithmetic::FieldExt,
    circuit::{AssignedCell, Layouter, Region, SimpleFloorPlanner, Value},
    plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Expression, Instance, Selector},
    poly::Rotation,
};

trait Params<F: FieldExt> {
    fn layer1_weights() -> [[F; 128]; 10];
    fn layer2_weights() -> [[F; 10]; 128];
}

struct NeuralNetwork<F: FieldExt, P: Params<F>> {
    layer1_values: [Value<F>; 10],
    layer2_values: [Value<F>; 128],
    layer3_values: [Value<F>; 10],
    _params: PhantomData<P>,
}

#[derive(Clone)]
struct NeuralNetworkConfig {
    node_columns: [Column<Advice>; 128],
    layer_selectors: [Selector; 3],
    output_column: Column<Instance>,
}

impl<F: FieldExt, P: Params<F>> Circuit<F> for NeuralNetwork<F, P> {
    type Config = NeuralNetworkConfig;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self {
            layer1_values: [Value::unknown(); 10],
            layer2_values: [Value::unknown(); 128],
            layer3_values: [Value::unknown(); 10],
            _params: PhantomData,
        }
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let node_columns = [(); 128].map(|_| meta.advice_column());
        let layer_selectors = [(); 3].map(|_| meta.selector());
        let output_column = meta.instance_column();

        // Constrain Layer 2
        meta.create_gate("layer_2", |v_cells| {
            let node_exps: [Expression<F>; 10] = (0..10)
                .map(|i| v_cells.query_advice(node_columns[i], Rotation::cur()))
                .collect::<Vec<Expression<F>>>()
                .try_into()
                .unwrap();
            let l1_weights = P::layer1_weights();
            let mut next_values = [(); 128].map(|_| Expression::Constant(F::zero()));

            for i in 0..10 {
                for j in 0..128 {
                    let next_v = node_exps[i].clone() * l1_weights[j][i];
                    next_values[j] = next_values[j].clone() + next_v;
                }
            }

            next_values.to_vec()
        });

        // Constrain Output Layer
        meta.create_gate("out_layer", |v_cells| {
            let node_exps: [Expression<F>; 128] = (0..128)
                .map(|i| v_cells.query_advice(node_columns[i], Rotation::cur()))
                .collect::<Vec<Expression<F>>>()
                .try_into()
                .unwrap();
            let l2_weights = P::layer2_weights();
            let mut next_values = [(); 10].map(|_| Expression::Constant(F::zero()));

            for i in 0..128 {
                for j in 0..10 {
                    let next_v = node_exps[i].clone() * l2_weights[j][i];
                    next_values[j] = next_values[j].clone() + next_v;
                }
            }

            next_values.to_vec()
        });

        NeuralNetworkConfig {
            node_columns,
            output_column,
            layer_selectors,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        // Assigne values in Layer 1
        layouter.assign_region(
            || "layer_1",
            |mut region: Region<'_, F>| {
                // Enable gates for Layer 1
                config.layer_selectors[0].enable(&mut region, 0)?;

                for i in 0..self.layer1_values.len() {
                    region.assign_advice(
                        || format!("layer1_node_{}", i),
                        config.node_columns[i],
                        0,
                        || self.layer1_values[i],
                    )?;
                }

                Ok(())
            },
        )?;

        // Assign values in Layer 2
        layouter.assign_region(
            || "layer_2",
            |mut region: Region<'_, F>| {
                // Enable gates for Layer 2
                config.layer_selectors[1].enable(&mut region, 0)?;

                for i in 0..self.layer2_values.len() {
                    region.assign_advice(
                        || format!("layer2_node_{}", i),
                        config.node_columns[i],
                        0,
                        || self.layer2_values[i],
                    )?;
                }

                Ok(())
            },
        )?;

        // Assign values in Layer 3
        let output_layer3 = layouter.assign_region(
            || "layer_2",
            |mut region: Region<'_, F>| {
                // Enable gates for Layer 3
                config.layer_selectors[2].enable(&mut region, 0)?;

                let mut output: [Option<AssignedCell<F, F>>; 10] = [(); 10].map(|_| None);
                for i in 0..self.layer3_values.len() {
                    let cell = region.assign_advice(
                        || format!("layer3_node_{}", i),
                        config.node_columns[i],
                        0,
                        || self.layer3_values[i],
                    )?;
                    output[i] = Some(cell);
                }

                Ok(output.map(|x| x.unwrap()))
            },
        )?;

        for i in 0..output_layer3.len() {
            layouter.constrain_instance(output_layer3[i].cell(), config.output_column, i)?;
        }
        Ok(())
    }
}

fn main() {
    println!("Hello, world!");
}
