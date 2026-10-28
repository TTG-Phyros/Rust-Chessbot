# Estimate

Size and cost of an architecture, to compare designs before training.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**accumulator_bytes** | **int** | Memory of one accumulator (both perspectives), copied at every ply. | 
**cost_vs_legacy** | **float** | &#x60;(update_ops + dense_macs)&#x60; relative to &#x60;legacy-768x256&#x60;: a rough proxy of the evaluation cost, not a measurement. | 
**dense_macs** | **int** | Multiply-adds of the dense layers per evaluation. | 
**input_features** | **int** | Input features per perspective. | 
**king_refreshes** | **bool** | Whether king moves can force refreshes. | 
**parameters** | **int** | Trainable parameters. | 
**refresh_ops** | **int** | i16 additions to rebuild one perspective from scratch (king bucket or mirror change). | 
**update_ops** | **int** | i16 additions to update the accumulators after a quiet move. | 
**weights_bytes** | **int** | Size of the quantized weights, in bytes (the network file adds a header of a few hundred bytes). | 

## Example

```python
from chess_nnue_lab.models.estimate import Estimate

# TODO update the JSON string below
json = "{}"
# create an instance of Estimate from a JSON string
estimate_instance = Estimate.from_json(json)
# print the JSON string representation of the object
print(Estimate.to_json())

# convert the object into a dict
estimate_dict = estimate_instance.to_dict()
# create an instance of Estimate from a dict
estimate_from_dict = Estimate.from_dict(estimate_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


