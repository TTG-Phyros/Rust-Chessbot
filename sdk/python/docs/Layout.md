# Layout

Resolved shapes of a (normalized) architecture.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**accumulator** | **int** | Accumulator neurons per perspective. | 
**accumulator_activation** | [**Activation**](Activation.md) | Accumulator activation. | 
**dense** | [**List[DenseShape]**](DenseShape.md) | Hidden layers, then the output layer. | 
**feature_set** | [**FeatureSet**](FeatureSet.md) | Feature encoding. | 
**ft_outputs** | **int** | Accumulator outputs per perspective after its activation. | 
**ft_weight_max** | **int** | Largest quantized feature transformer weight or bias. | 
**inputs** | **int** | Input features per perspective (&#x60;king_buckets × planes × 64&#x60;). | 
**king_buckets** | **int** | Distinct king buckets. | 
**king_refreshes** | **bool** | Whether a king move can force a full refresh of its perspective. | 
**max_active** | **int** | Most active features per perspective. | 
**output_buckets** | **int** | Output buckets. | 
**perspectives** | **int** | 2 with perspective, 1 without. | 
**planes** | **int** | Planes of 64 squares per king bucket. | 
**weight_max** | **int** | Largest quantized dense weight. | 

## Example

```python
from chess_nnue_lab.models.layout import Layout

# TODO update the JSON string below
json = "{}"
# create an instance of Layout from a JSON string
layout_instance = Layout.from_json(json)
# print the JSON string representation of the object
print(Layout.to_json())

# convert the object into a dict
layout_dict = layout_instance.to_dict()
# create an instance of Layout from a dict
layout_from_dict = Layout.from_dict(layout_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


