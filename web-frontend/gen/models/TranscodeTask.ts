/* tslint:disable */
/* eslint-disable */
/**
 * SegmentedEncoder rest api
 * No description provided (generated by Openapi Generator https://github.com/openapitools/openapi-generator)
 *
 * The version of the OpenAPI document: 0.0.1
 * 
 *
 * NOTE: This class is auto generated by OpenAPI Generator (https://openapi-generator.tech).
 * https://openapi-generator.tech
 * Do not edit the class manually.
 */

import { mapValues } from '../runtime';
import type { CodecParams } from './CodecParams';
import {
    CodecParamsFromJSON,
    CodecParamsFromJSONTyped,
    CodecParamsToJSON,
} from './CodecParams';

/**
 * 
 * @export
 * @interface TranscodeTask
 */
export interface TranscodeTask {
    /**
     * 
     * @type {CodecParams}
     * @memberof TranscodeTask
     */
    options: CodecParams;
}

/**
 * Check if a given object implements the TranscodeTask interface.
 */
export function instanceOfTranscodeTask(value: object): boolean {
    if (!('options' in value)) return false;
    return true;
}

export function TranscodeTaskFromJSON(json: any): TranscodeTask {
    return TranscodeTaskFromJSONTyped(json, false);
}

export function TranscodeTaskFromJSONTyped(json: any, ignoreDiscriminator: boolean): TranscodeTask {
    if (json == null) {
        return json;
    }
    return {
        
        'options': CodecParamsFromJSON(json['options']),
    };
}

export function TranscodeTaskToJSON(value?: TranscodeTask | null): any {
    if (value == null) {
        return value;
    }
    return {
        
        'options': CodecParamsToJSON(value['options']),
    };
}

