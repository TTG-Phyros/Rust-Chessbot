# Histogram

Counts of values in bins of equal width, with the values below the first bin in `below` and those past the last in `above`.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**above** | **int** | Values at or past &#x60;min + counts.len()·width&#x60;. | 
**below** | **int** | Values below &#x60;min&#x60;. | 
**counts** | **List[int]** | Values in &#x60;[min + i·width, min + (i+1)·width)&#x60;. | 
**min** | **int** | Lower bound of the first bin (inclusive). | 
**width** | **int** | Width of each bin. | 

## Example

```python
from chess_nnue_lab.models.histogram import Histogram

# TODO update the JSON string below
json = "{}"
# create an instance of Histogram from a JSON string
histogram_instance = Histogram.from_json(json)
# print the JSON string representation of the object
print(Histogram.to_json())

# convert the object into a dict
histogram_dict = histogram_instance.to_dict()
# create an instance of Histogram from a dict
histogram_from_dict = Histogram.from_dict(histogram_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


