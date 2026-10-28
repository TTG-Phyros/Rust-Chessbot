# DenseShape

A dense layer as stored: `buckets` sets of `outputs × inputs` weights.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**activation** | [**Activation**](Activation.md) | Activation after the layer; &#x60;None&#x60; for the output layer. | [optional] 
**buckets** | **int** | Sets of weights (1 when the layer is not bucketed). | 
**input_bound** | **int** | Largest quantized input value. | 
**inputs** | **int** | Inputs of one neuron. | 
**outputs** | **int** | Neurons. | 
**sum** | [**SumWidth**](SumWidth.md) | Integer type of the dot products. | 

## Example

```python
from chess_nnue_lab.models.dense_shape import DenseShape

# TODO update the JSON string below
json = "{}"
# create an instance of DenseShape from a JSON string
dense_shape_instance = DenseShape.from_json(json)
# print the JSON string representation of the object
print(DenseShape.to_json())

# convert the object into a dict
dense_shape_dict = dense_shape_instance.to_dict()
# create an instance of DenseShape from a dict
dense_shape_from_dict = DenseShape.from_dict(dense_shape_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


