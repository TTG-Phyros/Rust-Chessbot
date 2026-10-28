# DenseLayer

A dense (fully connected) layer.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**activation** | [**Activation**](Activation.md) | &#x60;ReLU&#x60;, &#x60;CReLU&#x60; or &#x60;SCReLU&#x60;. | 
**size** | **int** | Number of neurons. | 

## Example

```python
from chess_nnue_lab.models.dense_layer import DenseLayer

# TODO update the JSON string below
json = "{}"
# create an instance of DenseLayer from a JSON string
dense_layer_instance = DenseLayer.from_json(json)
# print the JSON string representation of the object
print(DenseLayer.to_json())

# convert the object into a dict
dense_layer_dict = dense_layer_instance.to_dict()
# create an instance of DenseLayer from a dict
dense_layer_from_dict = DenseLayer.from_dict(dense_layer_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


