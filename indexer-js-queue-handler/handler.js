import Indexer from "./indexer.js";
import AWSXRay from "aws-xray-sdk";
import AWS from "aws-sdk";

// Capture calls to AWS services in X-ray traces
AWSXRay.captureAWS(AWS);

export const consumer = async (event) => {
    const indexer = new Indexer('mainnet');

    for (const record of event.Records) {
        const jsonBody = JSON.parse(record.body);
        const block_height = jsonBody.block_height;
        const is_historical = jsonBody.is_historical;
        const functions = {};

        const function_config = jsonBody.indexer_function;
        const code = function_config.code;
        if (code.indexOf('context.db') >= 0) {
          continue
        }

        const function_name = function_config.account_id + '/' + function_config.function_name;
        functions[function_name] = function_config;

        try {
            const mutations = await indexer.runFunctions(block_height, functions, is_historical, {imperative: true, provision: true});
        } catch(e) {
            console.error(e);
        }
    }
};
