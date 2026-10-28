# Sprt

A sequential probability ratio test: is the first player at least `elo1` stronger (H1), or at most `elo0` (H0)?

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**alpha** | **float** | Probability of accepting H1 when H0 holds. | [optional] [default to 0.05]
**beta** | **float** | Probability of accepting H0 when H1 holds. | [optional] [default to 0.05]
**elo0** | **float** | Elo difference of H0. | [optional] [default to 0.0]
**elo1** | **float** | Elo difference of H1. | [optional] [default to 10.0]

## Example

```python
from chess_nnue_lab.models.sprt import Sprt

# TODO update the JSON string below
json = "{}"
# create an instance of Sprt from a JSON string
sprt_instance = Sprt.from_json(json)
# print the JSON string representation of the object
print(Sprt.to_json())

# convert the object into a dict
sprt_dict = sprt_instance.to_dict()
# create an instance of Sprt from a dict
sprt_from_dict = Sprt.from_dict(sprt_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


