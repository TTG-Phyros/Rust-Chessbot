# Quantization

Integer storage of the network.  Accumulator values are stored ×`qa` (1.0 ↔ `qa`), dense weights ×`qb`, dense biases ×`qa·qb` in i32. Every activation brings values back to the `qa` scale, and the network output is `sum · evalScale / (qa · qb)` centipawns.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**accumulator** | [**AccumulatorType**](AccumulatorType.md) | Accumulator (and feature transformer weights) type. | [optional] 
**eval_scale** | **int** | Centipawns per unit of network output (the trainer&#39;s sigmoid scale). | 
**ft_weight_clip** | **float** | Bound on feature transformer weights and biases during training, so that the i16 accumulator never overflows. Default: the largest safe value, &#x60;⌊32767 / (max active features + 1)⌋ / qa&#x60;. | [optional] 
**qa** | **int** | Scale of the accumulator: an activation of 1.0 is &#x60;qa&#x60;. | 
**qb** | **int** | Scale of the dense weights. | 
**weight_clip** | **float** | Bound on dense weights during training. Default: the largest value the weight type holds, &#x60;127 / qb&#x60; (i8) or &#x60;32767 / qb&#x60; (i16). | [optional] 
**weights** | [**WeightType**](WeightType.md) | Type of the dense layers&#39; weights. | 

## Example

```python
from chess_nnue_lab.models.quantization import Quantization

# TODO update the JSON string below
json = "{}"
# create an instance of Quantization from a JSON string
quantization_instance = Quantization.from_json(json)
# print the JSON string representation of the object
print(Quantization.to_json())

# convert the object into a dict
quantization_dict = quantization_instance.to_dict()
# create an instance of Quantization from a dict
quantization_from_dict = Quantization.from_dict(quantization_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


